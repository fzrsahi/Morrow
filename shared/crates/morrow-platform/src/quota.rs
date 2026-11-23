//! Claude plan-limit, OS half (#306): the file and the wire.
//!
//! [`morrow_core::quota`] owns the limit model, the usage-response parser and
//! the refresh policy. This module owns the two things that touch the machine:
//! reading Claude Code's login from `~/.claude/.credentials.json`, and running
//! `curl` to fetch `https://api.anthropic.com/api/oauth/usage`. Both are handed
//! their inputs (a home path, an injected [`Runner`], every instant), so a test
//! never needs a real login, a clock or a network.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use morrow_core::quota::{Credential, QuotaError, Scheduler, Snapshot, interpret_response};
use morrow_core::reduce::Timestamp;
use serde_json::Value;

use crate::runner::Runner;

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";

/// Read Claude Code's OAuth login. A missing file means "not signed in", not
/// an error; a file that cannot yield a token is an error. The token is only
/// ever copied into the returned [`Credential`] — it is never logged, and the
/// core type redacts it from `Debug` and `Serialize`. Nothing is written here.
pub fn read_credentials(home: &Path) -> Result<Option<Credential>, String> {
    let path = home.join(".claude").join(".credentials.json");
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("{}: not valid JSON: {error}", path.display()))?;
    let oauth = value
        .get("claudeAiOauth")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{}: no claudeAiOauth object", path.display()))?;
    let access_token = oauth
        .get("accessToken")
        .and_then(Value::as_str)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| format!("{}: no non-empty accessToken", path.display()))?;
    // A known-expired token is returned as-is on purpose: the core credential
    // reports it as unauthorized before any request is sent.
    Ok(Some(Credential {
        access_token: access_token.to_owned(),
        subscription_type: oauth
            .get("subscriptionType")
            .and_then(Value::as_str)
            .map(str::to_owned),
        expires_at: oauth
            .get("expiresAt")
            .and_then(Value::as_f64)
            .map(|milliseconds| Timestamp(milliseconds / 1000.0)),
    }))
}

/// The outer `Err` is a local/transport failure; the inner result is exactly
/// what [`interpret_response`] says about the bytes the shell collected.
pub type FetchResult = Result<Result<Snapshot, QuotaError>, String>;

/// Scrub the bearer token out of anything a tool might have echoed back.
fn redact(text: &str, token: &str) -> String {
    // `String::replace("", …)` inserts everywhere; an empty token is not a
    // secret and needs no redaction.
    if token.is_empty() {
        return text.to_owned();
    }
    text.replace(token, "<redacted>")
}

/// One usage request through the injected runner. `now` is the instant stamped
/// onto the snapshot — passed in, never read from the wall clock here.
pub fn fetch(runner: &dyn Runner, credential: &Credential, now: Timestamp) -> FetchResult {
    // Swift `fetch(using:)`: a token we know is expired would only be rejected
    // by the server, so report unauthorized without putting it on the wire.
    if credential.is_expired(now) {
        return Ok(Err(QuotaError::Unauthorized));
    }

    let token = &credential.access_token;
    // #306 AC: the token must never appear in recorded arguments. curl reads
    // its config from stdin, so the Authorization header is a config line,
    // not an argv element visible in `ps`. The injected Runner's
    // `run_with_stdin` is the only channel that carries the secret.
    let config = format!("header = \"Authorization: Bearer {token}\"\n");
    let args = [
        "--silent",
        "--show-error",
        "--max-time",
        "10",
        // Refuse redirects: the bearer token must never be replayed to a
        // different host. A 3xx is reported as `QuotaError::Http` instead.
        "--max-redirs",
        "0",
        "--config",
        "-",
        "-w",
        "\\n%{http_code}",
        "-X",
        "GET",
        USAGE_URL,
    ];
    // No token in argv and nothing to redact from it; errors still pass through
    // `redact` in case a tool echoes the config back on stderr.
    let output = runner
        .run_with_stdin("curl", &args, &config)
        .map_err(|error| format!("usage request failed: {}", redact(&error, token)))?;

    // `-w '\n%{http_code}'` appends one line holding the status. Splitting at
    // the last newline keeps bodies that end in a newline intact.
    let (body, status_line) = output
        .rsplit_once('\n')
        .ok_or_else(|| "curl wrote no HTTP status line".to_owned())?;
    let status: i32 = status_line.trim().parse().map_err(|_| {
        format!(
            "curl wrote a non-numeric HTTP status: {}",
            redact(status_line, token)
        )
    })?;

    Ok(interpret_response(body.as_bytes(), status, now))
}

/// What [`QuotaFetcher::fetch_if_due`] decided to do.
#[derive(Debug)]
pub enum FetchAction {
    /// Nothing on screen, a login is needed, or the next fetch is not due yet.
    Skipped,
    /// An attempt happened — including the known-expired short circuit — and
    /// this is the core's interpretation of it.
    Fetched(Result<Snapshot, QuotaError>),
    /// A local/transport failure, scrubbed of the bearer token.
    Error(String),
}

/// The scheduler plus the two OS seams, owned together. Every instant comes
/// from the caller; nothing here starts a timer or reads the clock.
pub struct QuotaFetcher<R: Runner> {
    pub runner: R,
    pub home: PathBuf,
    pub scheduler: Scheduler,
}

impl<R: Runner> QuotaFetcher<R> {
    pub fn new(runner: R, home: PathBuf) -> Self {
        Self {
            runner,
            home,
            scheduler: Scheduler::default(),
        }
    }

    /// The earliest background fetch, straight from the core policy.
    pub fn next_fire_time(&self, wants_live: bool, now: Timestamp) -> Option<Timestamp> {
        self.scheduler.next_fire_time(wants_live, now)
    }

    /// Re-read the login from this fetcher's home (a rotated token is picked
    /// up without rebuilding the fetcher).
    pub fn read_credentials(&self) -> Result<Option<Credential>, String> {
        read_credentials(&self.home)
    }

    /// Fetch only when the core scheduler says it is due, then feed the result
    /// back into it so failures back off and a rejected token pauses until the
    /// user expands again.
    pub fn fetch_if_due(
        &mut self,
        now: Timestamp,
        credential: &Credential,
        wants_live: bool,
    ) -> FetchAction {
        let Some(due) = self.scheduler.next_fire_time(wants_live, now) else {
            return FetchAction::Skipped;
        };
        if now.0 < due.0 {
            return FetchAction::Skipped;
        }
        self.scheduler.record_fetch_start(now);

        // Swift `fetch(using:)`: never send a token already known to be expired.
        if credential.is_expired(now) {
            self.scheduler.record_failure(true);
            return FetchAction::Fetched(Err(QuotaError::Unauthorized));
        }

        match fetch(&self.runner, credential, now) {
            Ok(Ok(snapshot)) => {
                self.scheduler.record_success();
                FetchAction::Fetched(Ok(snapshot))
            }
            Ok(Err(QuotaError::Unauthorized)) => {
                self.scheduler.record_failure(true);
                FetchAction::Fetched(Err(QuotaError::Unauthorized))
            }
            Ok(Err(error)) => {
                self.scheduler.record_failure(false);
                FetchAction::Fetched(Err(error))
            }
            Err(error) => {
                self.scheduler.record_failure(false);
                FetchAction::Error(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    const TOKEN: &str = "sk-ant-oat01-very-secret";

    fn temp_home(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("a clock")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("morrow-quota-{tag}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&root).expect("temp home");
        root
    }

    fn credential_path(home: &Path) -> PathBuf {
        home.join(".claude").join(".credentials.json")
    }

    fn write_credentials(home: &Path, json: &str) {
        let path = credential_path(home);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, json).unwrap();
    }

    fn credential() -> Credential {
        Credential {
            access_token: TOKEN.to_owned(),
            subscription_type: None,
            expires_at: None,
        }
    }

    fn now() -> Timestamp {
        Timestamp(1_800_000_000.0)
    }

    fn oauth_json() -> String {
        format!(
            r#"{{"claudeAiOauth":{{"accessToken":"{TOKEN}","subscriptionType":"max","expiresAt":1788472382000}}}}"#
        )
    }

    /// Records every call so tests can inspect the argv without a shell.
    struct FakeRunner {
        response: String,
        error: Option<String>,
        calls: RefCell<Vec<(String, Vec<String>)>>,
        stdin_log: RefCell<Vec<String>>,
    }

    impl FakeRunner {
        fn ok(body: &str, status: i32) -> Self {
            Self {
                response: format!("{body}\n{status}"),
                error: None,
                calls: RefCell::new(Vec::new()),
                stdin_log: RefCell::new(Vec::new()),
            }
        }

        fn failing(error: String) -> Self {
            Self {
                response: String::new(),
                error: Some(error),
                calls: RefCell::new(Vec::new()),
                stdin_log: RefCell::new(Vec::new()),
            }
        }

        fn call_count(&self) -> usize {
            self.calls.borrow().len()
        }
    }

    impl Runner for FakeRunner {
        fn run(&self, program: &str, args: &[&str]) -> Result<String, String> {
            self.run_with_stdin(program, args, "")
        }

        fn run_with_stdin(
            &self,
            program: &str,
            args: &[&str],
            stdin: &str,
        ) -> Result<String, String> {
            self.calls.borrow_mut().push((
                program.to_owned(),
                args.iter().map(|arg| (*arg).to_owned()).collect(),
            ));
            self.stdin_log.borrow_mut().push(stdin.to_owned());
            match &self.error {
                Some(error) => Err(error.clone()),
                None => Ok(self.response.clone()),
            }
        }
    }

    #[test]
    fn credentials_are_read_and_an_absent_file_is_none() {
        let home = temp_home("creds-ok");
        write_credentials(&home, &oauth_json());
        let found = read_credentials(&home).unwrap().unwrap();
        assert_eq!(found.access_token, TOKEN);
        assert_eq!(found.subscription_type.as_deref(), Some("max"));
        assert_eq!(found.expires_at.unwrap().0, 1_788_472_382.0);

        let empty = temp_home("creds-absent");
        assert!(read_credentials(&empty).unwrap().is_none());
        assert!(
            !credential_path(&empty).exists(),
            "the reader must not create it"
        );
    }

    #[test]
    fn malformed_credentials_are_an_error_not_a_silent_none() {
        let home = temp_home("creds-bad");
        for json in [
            "not json",
            "{}",
            r#"{"claudeAiOauth":{"subscriptionType":"max"}}"#,
            r#"{"claudeAiOauth":{"accessToken":""}}"#,
        ] {
            write_credentials(&home, json);
            let error = read_credentials(&home).unwrap_err();
            assert!(!error.contains(TOKEN), "{error}");
        }
    }

    #[test]
    fn transport_errors_never_echo_the_bearer_token() {
        let runner = FakeRunner::failing(format!("curl failed, argv was [Bearer {TOKEN}]"));
        let error = fetch(&runner, &credential(), now()).unwrap_err();
        assert!(!error.contains(TOKEN), "{error}");
        assert!(error.contains("<redacted>"), "{error}");
    }

    #[test]
    fn statuses_are_interpreted_by_core_without_following_redirects() {
        let body = r#"{"five_hour":{"utilization":3.0,"resets_at":"2026-09-03T19:50:00Z"}}"#;
        for (name, response_body, status, expected) in [
            ("ok", body.to_owned(), 200, None),
            ("redirect", String::new(), 302, Some(QuotaError::Http(302))),
            (
                "unauthorized",
                String::new(),
                401,
                Some(QuotaError::Unauthorized),
            ),
            (
                "rate limited",
                String::new(),
                429,
                Some(QuotaError::RateLimited),
            ),
        ] {
            let runner = FakeRunner::ok(&response_body, status);
            let outcome = fetch(&runner, &credential(), now()).unwrap();
            assert_eq!(outcome.err(), expected, "{name}");
        }

        let runner = FakeRunner::ok(body, 200);
        fetch(&runner, &credential(), now()).unwrap().unwrap();
        let calls = runner.calls.borrow();
        let (program, args) = &calls[0];
        assert_eq!(program, "curl");
        assert!(
            args.windows(2)
                .any(|pair| pair[0] == "--max-redirs" && pair[1] == "0")
        );
        assert!(
            !args
                .iter()
                .any(|arg| arg.as_str() == "-L" || arg.as_str() == "--location"),
            "{args:?}"
        );
        for required in [
            "--silent",
            "--show-error",
            "--max-time",
            "10",
            "-X",
            "GET",
            "-w",
        ] {
            assert!(
                args.iter().any(|arg| arg.as_str() == required),
                "{required}"
            );
        }
        assert!(args.iter().any(|arg| arg.as_str() == "\\n%{http_code}"));
        // #306: the bearer header goes through stdin, never argv.
        assert!(
            !args
                .iter()
                .any(|arg| arg.contains(TOKEN) || arg.contains("Bearer")),
            "token leaked into argv: {args:?}"
        );
        assert!(
            args.windows(2)
                .any(|pair| pair[0] == "--config" && pair[1] == "-"),
            "{args:?}"
        );
        let config = runner.stdin_log.borrow();
        let header = format!("header = \"Authorization: Bearer {TOKEN}\"");
        assert_eq!(
            config.first().map(|line| line.trim()),
            Some(header.as_str())
        );
    }

    #[test]
    fn fetcher_skips_when_disabled_and_pauses_after_unauthorized() {
        let runner = FakeRunner::ok("", 401);
        let mut fetcher = QuotaFetcher::new(runner, temp_home("skip"));
        let credential = credential();

        assert!(matches!(
            fetcher.fetch_if_due(now(), &credential, false),
            FetchAction::Skipped
        ));
        assert_eq!(fetcher.runner.call_count(), 0);

        assert!(matches!(
            fetcher.fetch_if_due(now(), &credential, true),
            FetchAction::Fetched(Err(QuotaError::Unauthorized))
        ));
        assert_eq!(fetcher.runner.call_count(), 1);
        assert!(fetcher.scheduler.needs_login);
        assert_eq!(fetcher.next_fire_time(true, now()), None);
        assert!(matches!(
            fetcher.fetch_if_due(now(), &credential, true),
            FetchAction::Skipped
        ));
        assert_eq!(
            fetcher.runner.call_count(),
            1,
            "a rejected token stops fetches"
        );
    }

    #[test]
    fn a_known_expired_credential_never_hits_the_network() {
        let runner = FakeRunner::ok("", 200);
        let mut fetcher = QuotaFetcher::new(runner, temp_home("expired"));
        let credential = Credential {
            expires_at: Some(Timestamp(now().0 - 1.0)),
            ..credential()
        };

        assert!(matches!(
            fetcher.fetch_if_due(now(), &credential, true),
            FetchAction::Fetched(Err(QuotaError::Unauthorized))
        ));
        assert_eq!(fetcher.runner.call_count(), 0);
    }

    #[test]
    fn failures_back_off_and_success_clears_them() {
        let runner = FakeRunner::ok("", 503);
        let mut fetcher = QuotaFetcher::new(runner, temp_home("backoff"));
        let credential = credential();

        assert!(matches!(
            fetcher.fetch_if_due(now(), &credential, true),
            FetchAction::Fetched(Err(QuotaError::Http(503)))
        ));
        assert_eq!(fetcher.scheduler.failures, 1);
        assert_eq!(fetcher.scheduler.backoff(), Some(60.0));
        assert!(!fetcher.scheduler.needs_login);

        fetcher.runner.response = format!("{}\n200", r#"{"five_hour":{"utilization":3.0}}"#);
        let later = Timestamp(now().0 + 700.0);
        assert!(matches!(
            fetcher.fetch_if_due(later, &credential, true),
            FetchAction::Fetched(Ok(_))
        ));
        assert_eq!(fetcher.scheduler.failures, 0);
        assert_eq!(fetcher.scheduler.backoff(), None);
    }

    #[test]
    fn transport_errors_are_reported_and_counted() {
        let runner = FakeRunner::failing("curl exploded".to_owned());
        let mut fetcher = QuotaFetcher::new(runner, temp_home("transport"));

        assert!(matches!(
            fetcher.fetch_if_due(now(), &credential(), true),
            FetchAction::Error(message) if message.contains("curl exploded")
        ));
        assert_eq!(fetcher.scheduler.failures, 1);
    }
}
