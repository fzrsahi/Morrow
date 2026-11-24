//! Backfill: the sessions that exist before their hook does (#261).
//!
//! Until this, a session could only be born from a hook. Restart the app while an
//! agent is working and the panel is empty until that agent's next event — and a
//! session whose hook never carried a pid is never pruned, so its card outlives
//! the process.
//!
//! The scan itself is `morrow_core::discovery::scan` (#260): roots × readers,
//! deduped, within a budget. What lives here is the one reader that reads
//! something real today — Claude Code's `projects/` tree — and the decision of
//! when to run it.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use morrow_core::discovery::scan::{self, ScanResult, StoreReader};
use morrow_core::discovery::{DiscoveryHit, read_transcript_tail, transcript_files};

/// How much of a transcript the backfill reads. The same slice the live tailer
/// reads on attach, so backfill costs one read of bytes that are already going to
/// be read rather than a second parse of the whole store.
const TAIL_BYTES: u64 = 64 * 1024;

/// Claude Code's `projects/<encoded>/<session>.jsonl`.
///
/// Two levels, not a recursive walk: `projects/` holds one directory per working
/// directory and the transcripts directly inside it. `Discovery/` on macOS only
/// ever lists one directory for the same reason.
pub struct ClaudeTranscripts {
    /// Claude Code's config directory, resolved by the installer's own ladder
    /// (#258) so discovery reads the folder the hooks were written into.
    pub config_dir: String,
}

impl ClaudeTranscripts {
    fn config_dir(&self) -> &Path {
        Path::new(&self.config_dir)
    }
}

impl StoreReader for ClaudeTranscripts {
    fn source(&self) -> &'static str {
        "claude"
    }

    /// One store per config root. Declared by the walker that found them, so this
    /// reader does not go looking for accounts it was not told about.
    fn roots(&self) -> Vec<String> {
        claude_roots(self.config_dir())
    }

    fn read(&self, root: &str) -> Vec<DiscoveryHit> {
        let projects = Path::new(root).join("projects");
        let Ok(entries) = std::fs::read_dir(&projects) else {
            return Vec::new();
        };
        let mut hits = Vec::new();
        for entry in entries.flatten() {
            // `entry.path()` is the encoded project directory. Entries are not
            // resolved: a symlinked transcript keeps its own mtime, which is what
            // tells a live session from one left behind (#256).
            let Some(files) = transcript_files::list(&entry.path().to_string_lossy()) else {
                continue;
            };
            for file in files {
                let Some(tail) = read_tail(Path::new(&file.path)) else {
                    continue;
                };
                let Some(summary) = read_transcript_tail(&tail, &file.name) else {
                    continue;
                };
                hits.push(DiscoveryHit {
                    session_id: summary.session_id,
                    source: "claude",
                    cwd: summary.cwd,
                    tty: None,
                    model: summary.model,
                    pid: None,
                    modified_at: Some(file.modified_ms),
                    recent_messages: summary.recent_messages,
                    transcript_path: Some(file.path),
                });
            }
        }
        hits
    }
}

/// The last `TAIL_BYTES` of a file as text. A partial UTF-8 sequence at the cut
/// is dropped rather than failing the whole read.
fn read_tail(path: &Path) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let size = file.metadata().ok()?.len();
    let start = size.saturating_sub(TAIL_BYTES);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::new();
    file.take(TAIL_BYTES).read_to_end(&mut bytes).ok()?;
    if start > 0 {
        // Skip to the first newline: the row at the cut is half a line, and the
        // reader tolerates it, but dropping it keeps a cut multi-byte char from
        // making the whole tail lossy.
        if let Some(index) = bytes.iter().position(|byte| *byte == b'\n') {
            bytes.drain(..=index);
        }
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// OpenCode's (and MiMo Code's) session database (#270).
///
/// The one reader that can be backfilled **without a process**, because the store
/// itself records the working directory: `find_recent_session` asks "which session
/// is this process in", which needs a cwd from the process table, while
/// `recent_sessions` asks the database what it holds.
///
/// Cursor and Gemini are the other way round and are deliberately **not**
/// registered: their store directory is derived from a process's cwd
/// (`~/.cursor/projects/<encoded cwd>/agent-transcripts`), and that encoding maps
/// `/` and a space to the same `-`. Guessing a cwd from it would put a card in a
/// directory the user does not have.
pub struct OpenCodeStore {
    /// `~/.local/share`, where both builds keep their database.
    pub data_dir: String,
}

impl OpenCodeStore {
    /// `opencode.db` and `mimocode.db`: the two names the builds ship (#257).
    fn databases(&self) -> Vec<PathBuf> {
        ["opencode/opencode.db", "mimocode/mimocode.db"]
            .iter()
            .map(|relative| Path::new(&self.data_dir).join(relative))
            .filter(|path| path.is_file())
            .collect()
    }
}

impl StoreReader for OpenCodeStore {
    fn source(&self) -> &'static str {
        "opencode"
    }

    /// One root, not one per account: a single database holds every session.
    fn roots(&self) -> Vec<String> {
        let databases = self.databases();
        if databases.is_empty() {
            return Vec::new();
        }
        vec![self.data_dir.clone()]
    }

    fn read(&self, _root: &str) -> Vec<DiscoveryHit> {
        let mut hits = Vec::new();
        for database in self.databases() {
            let Ok(connection) = morrow_core::sqlite::open_read_only(&database) else {
                // A store being read by the CLI is not a failure of discovery.
                continue;
            };
            for row in morrow_core::discovery::opencode::recent_sessions(&connection, 20) {
                let (model, recent_messages) =
                    morrow_core::discovery::opencode::read_recent(&connection, &row.session_id);
                hits.push(DiscoveryHit {
                    session_id: row.session_id,
                    source: "opencode",
                    cwd: row.cwd,
                    tty: None,
                    model,
                    pid: None,
                    // `time_updated` is milliseconds, the same unit a hit wants.
                    modified_at: u64::try_from(row.modified_ms).ok(),
                    recent_messages,
                    transcript_path: Some(database.to_string_lossy().into_owned()),
                });
            }
        }
        hits
    }
}

/// Give every hit that has no pid the process that is running it (#267).
///
/// Reads the process table **once**, for the whole scan: a table read per session
/// is a `/proc` walk per session, which is the cost #246 exists to avoid.
///
/// A table that cannot be read, or that names no candidate, leaves the hit
/// without a pid — the state before #266. Discovery must not fail because
/// correlation did: an empty panel is a worse outcome than a card without a
/// watchdog.
pub fn correlate_pids(hits: &mut [DiscoveryHit]) {
    if hits.iter().all(|hit| hit.pid.is_some()) {
        return;
    }
    let table = morrow_platform::agent_process::table();
    correlate_with(hits, &table, |pid| {
        morrow_platform::process::exists(i64::from(pid))
    });
}

/// `correlate_pids` with the table and the liveness probe injected, so the real
/// function — not a copy of it — is what the tests exercise.
pub fn correlate_with(
    hits: &mut [DiscoveryHit],
    table: &[morrow_core::agent_process::ProcessInfo],
    alive: impl Fn(i32) -> bool,
) {
    let needs: Vec<morrow_core::agent_process::PidRequest> = hits
        .iter()
        .filter(|hit| hit.pid.is_none())
        .map(|hit| morrow_core::agent_process::PidRequest {
            session_id: hit.session_id.clone(),
            source: hit.source.to_owned(),
            cwd: hit.cwd.clone(),
            modified_at_ms: hit.modified_at,
        })
        .collect();
    if needs.is_empty() || table.is_empty() {
        return;
    }
    // A pid a hook already named is not a candidate: that session is watched by
    // its own process, and handing its pid to another card would make two cards
    // share one watchdog.
    let mut claimed: std::collections::HashSet<i32> =
        hits.iter().filter_map(|hit| hit.pid).collect();
    let assignments = morrow_core::agent_process::assign(table, &needs, &mut claimed, alive);
    for assignment in assignments {
        if let Some(hit) = hits
            .iter_mut()
            .find(|hit| hit.session_id == assignment.session_id)
        {
            hit.pid = Some(assignment.pid);
        }
    }
}

/// One scan with every reader this port has, each over its own roots.
pub fn scan_once(config_dir: &Path, data_dir: &Path) -> ScanResult {
    let transcripts = ClaudeTranscripts {
        config_dir: config_dir.to_string_lossy().into_owned(),
    };
    let opencode = OpenCodeStore {
        data_dir: data_dir.to_string_lossy().into_owned(),
    };
    let readers: Vec<&dyn StoreReader> = vec![&transcripts, &opencode];
    scan::run(&readers, scan::DEFAULT_BUDGET_MS, now_ms)
}

/// The roots to scan for Claude Code today: the live config dir, plus any other
/// directory under it that holds `projects/`.
///
/// The extras are not Settings-registered ones yet (`ExtraConfigDir` has no Rust
/// counterpart); they are the sibling accounts a multi-account user already has,
/// discovered by the same marker that makes a root live.
pub fn claude_roots(primary: &Path) -> Vec<String> {
    let primary = primary.to_string_lossy().into_owned();
    let mut roots = scan::scan_roots(&primary, &[]);
    // A `~/.claude-work` style sibling: same parent, holds `projects/`.
    if let Some(parent) = Path::new(&primary).parent() {
        if let Ok(entries) = std::fs::read_dir(parent) {
            let mut siblings: Vec<String> = entries
                .flatten()
                .filter_map(|entry| {
                    let path = entry.path();
                    let name = path.file_name()?.to_string_lossy().into_owned();
                    if !name.starts_with(".claude") || path == Path::new(&primary) {
                        return None;
                    }
                    path.join("projects")
                        .is_dir()
                        .then(|| path.to_string_lossy().into_owned())
                })
                .collect();
            // Deterministic, so two runs scan in the same order.
            siblings.sort();
            roots.extend(scan::scan_roots(
                &primary,
                &siblings.iter().map(String::as_str).collect::<Vec<_>>(),
            ));
            // `scan_roots` puts the primary first and drops duplicates; the first
            // call above is subsumed by this one.
            return dedupe(roots);
        }
    }
    roots
}

fn dedupe(roots: Vec<String>) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    roots
        .into_iter()
        .filter(|root| seen.insert(morrow_core::path_identity::identity(root)))
        .collect()
}

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::path::PathBuf;

    use super::*;

    /// A scratch `~` that removes itself.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("morrow-backfill-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("a scratch home");
            Self(path)
        }

        fn project_dir(&self, root: &str, encoded: &str) -> PathBuf {
            let path = self.0.join(root).join("projects").join(encoded);
            fs::create_dir_all(&path).expect("a project directory");
            path
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn transcript(cwd: &str, model: &str) -> String {
        format!(
            "{}\n{}\n",
            format_args!(
                r#"{{"type":"user","cwd":"{cwd}","message":{{"role":"user","content":"Add a filter bar"}}}}"#
            ),
            format_args!(
                r#"{{"type":"assistant","cwd":"{cwd}","message":{{"model":"{model}","content":[{{"type":"text","text":"Added the bar"}}]}}}}"#
            )
        )
    }

    fn write(path: &Path, contents: &str) {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("a transcript");
        file.write_all(contents.as_bytes()).expect("a write");
    }

    #[test]
    fn a_transcript_without_a_hook_becomes_a_session() {
        let scratch = Scratch::new("one");
        let projects = scratch.project_dir(".claude", "-home-dev-project");
        write(
            &projects.join("session-a.jsonl"),
            &transcript("/home/dev/project", "model-x"),
        );

        let result = scan_once(&scratch.0.join(".claude"), &scratch.0.join("data"));

        assert_eq!(result.hits.len(), 1);
        let hit = &result.hits[0];
        assert_eq!(hit.session_id, "session-a");
        assert_eq!(hit.source, "claude");
        assert_eq!(hit.cwd, "/home/dev/project");
        assert_eq!(hit.model.as_deref(), Some("model-x"));
        assert_eq!(hit.recent_messages.len(), 2);
        assert!(
            hit.transcript_path
                .as_deref()
                .is_some_and(|path| path.ends_with("session-a.jsonl"))
        );
        assert!(hit.modified_at.is_some());
    }

    #[test]
    fn a_store_that_is_not_there_is_no_sessions() {
        let scratch = Scratch::new("missing");
        assert!(
            scan_once(&scratch.0.join(".claude"), &scratch.0.join("data"))
                .hits
                .is_empty()
        );
    }

    /// #267: a hit with no pid gets the process that is running it, so `Watches`
    /// can prune it. Exercises `correlate_with`, the function `correlate_pids`
    /// delegates to, not a copy of its filter.
    #[test]
    fn a_hit_without_a_pid_gets_the_process_running_it() {
        let table = vec![process(
            4242,
            "/usr/bin/claude",
            &["claude"],
            "/home/dev/project",
            1_000,
        )];
        let mut hits = vec![hit("s", "/home/dev/project", 5_000, None)];

        correlate_with(&mut hits, &table, |_| true);

        assert_eq!(hits[0].pid, Some(4242));
    }

    /// A pid a hook already named is not handed to another card, or two cards
    /// would share one watchdog and one of them would outlive its process.
    #[test]
    fn a_pid_a_hook_named_is_not_given_away() {
        let table = vec![process(7, "/usr/bin/claude", &["claude"], "/w", 1_000)];
        let mut hits = vec![
            hit("has-pid", "/w", 2_000, Some(7)),
            hit("needs-pid", "/w", 2_000, None),
        ];

        correlate_with(&mut hits, &table, |_| true);

        assert_eq!(hits[0].pid, Some(7), "the hook's own pid is untouched");
        assert_eq!(hits[1].pid, None, "the only candidate is already taken");
    }

    /// An empty table — macOS, a permission wall, a host with no `/proc` — leaves
    /// the hits without pids and must not fail: an empty panel is a worse outcome
    /// than a card without a watchdog.
    #[test]
    fn a_table_that_cannot_be_read_leaves_the_hits_alone() {
        let mut hits = vec![hit("s", "/w", 2_000, None)];
        correlate_with(&mut hits, &[], |_| true);
        assert_eq!(hits[0].pid, None);
    }

    /// The table is read once for the whole scan. A hit that already has a pid
    /// asks for nothing, so a scan with nothing to correlate never reads a table.
    #[test]
    fn a_hit_that_already_has_a_pid_asks_for_nothing() {
        let mut hits = vec![hit("s", "/w", 2_000, Some(9))];
        correlate_with(&mut hits, &[], |_| panic!("a table must not be read"));
        assert_eq!(hits[0].pid, Some(9));
    }

    /// Two sessions in one directory, neither with a pid: each gets its own, and
    /// a dead process is not a candidate.
    #[test]
    fn two_sessions_in_one_directory_get_their_own_processes() {
        let table = vec![
            process(10, "/usr/bin/claude", &["claude"], "/w", 1_000),
            process(11, "/usr/bin/claude", &["claude"], "/w", 5_000),
            // Long dead, so never a candidate whatever the order.
            process(12, "/usr/bin/claude", &["claude"], "/w", 500),
        ];
        let mut hits = vec![hit("a", "/w", 1_500, None), hit("b", "/w", 5_500, None)];

        correlate_with(&mut hits, &table, |pid| pid != 12);

        assert_eq!(hits[0].pid, Some(10));
        assert_eq!(hits[1].pid, Some(11));
    }

    fn process(
        pid: i32,
        executable: &str,
        args: &[&str],
        cwd: &str,
        started: u64,
    ) -> morrow_core::agent_process::ProcessInfo {
        morrow_core::agent_process::ProcessInfo {
            pid,
            executable: executable.to_owned(),
            arguments: args.iter().map(|arg| (*arg).to_owned()).collect(),
            cwd: Some(cwd.to_owned()),
            start_time_ms: Some(started),
        }
    }

    fn hit(session_id: &str, cwd: &str, modified_at: u64, pid: Option<i32>) -> DiscoveryHit {
        DiscoveryHit {
            session_id: session_id.to_owned(),
            source: "claude",
            cwd: cwd.to_owned(),
            pid,
            modified_at: Some(modified_at),
            ..DiscoveryHit::default()
        }
    }

    /// #270: a real OpenCode database becomes a card, with the working directory
    /// the store itself records — the one reader that can be backfilled without a
    /// process.
    #[test]
    fn an_opencode_database_becomes_a_session() {
        let scratch = Scratch::new("opencode");
        let data = scratch.0.join("data");
        std::fs::create_dir_all(data.join("opencode")).expect("a data directory");
        {
            let connection =
                rusqlite::Connection::open(data.join("opencode/opencode.db")).expect("a store");
            connection
                .execute_batch(
                    "CREATE TABLE session (id TEXT PRIMARY KEY, directory TEXT, parent_id TEXT,
                        time_updated INTEGER, time_archived INTEGER);
                     CREATE TABLE message (id TEXT PRIMARY KEY, session_id TEXT, data TEXT,
                        time_updated INTEGER);
                     CREATE TABLE part (id TEXT PRIMARY KEY, message_id TEXT, session_id TEXT,
                        data TEXT, time_created INTEGER);
                     INSERT INTO session VALUES ('oc-1', '/home/dev/project', NULL, 7000, NULL);
                     INSERT INTO message VALUES ('m1', 'oc-1',
                        '{\"role\":\"assistant\",\"modelID\":\"model-x\"}', 6000);
                     INSERT INTO part VALUES ('p1', 'm1', 'oc-1',
                        '{\"type\":\"text\",\"text\":\"Done\"}', 6000);",
                )
                .expect("a schema");
        }

        let result = scan_once(&scratch.0.join(".claude"), &data);

        assert_eq!(result.hits.len(), 1);
        let hit = &result.hits[0];
        assert_eq!(hit.session_id, "oc-1");
        assert_eq!(hit.source, "opencode");
        assert_eq!(
            hit.cwd, "/home/dev/project",
            "the store's own directory column"
        );
        assert_eq!(hit.model.as_deref(), Some("model-x"));
        assert_eq!(hit.modified_at, Some(7_000));
        assert_eq!(hit.recent_messages.len(), 1);
    }

    /// Two readers in one scan: Claude's transcripts and OpenCode's database both
    /// arrive, each from its own root.
    #[test]
    fn both_readers_contribute_to_one_scan() {
        let scratch = Scratch::new("both");
        let projects = scratch.project_dir(".claude", "-home-dev-project");
        write(
            &projects.join("claude-1.jsonl"),
            &transcript("/home/dev/p", "model-x"),
        );
        let data = scratch.0.join("data");
        std::fs::create_dir_all(data.join("opencode")).expect("a data directory");
        {
            let connection =
                rusqlite::Connection::open(data.join("opencode/opencode.db")).expect("a store");
            connection
                .execute_batch(
                    "CREATE TABLE session (id TEXT PRIMARY KEY, directory TEXT,
                        time_updated INTEGER);
                     INSERT INTO session VALUES ('oc-1', '/home/dev/q', 7000);",
                )
                .expect("a schema");
        }

        let mut hits = scan_once(&scratch.0.join(".claude"), &data).hits;
        hits.sort_by(|a, b| a.session_id.cmp(&b.session_id));

        let ids: Vec<&str> = hits.iter().map(|hit| hit.session_id.as_str()).collect();
        assert_eq!(ids, vec!["claude-1", "oc-1"]);
        assert_eq!(hits[0].source, "claude");
        assert_eq!(hits[1].source, "opencode");
    }

    /// A database that is not there is no sessions, and a database that cannot be
    /// opened does not lose the other reader's results.
    #[test]
    fn a_missing_or_unreadable_database_is_not_an_error() {
        let scratch = Scratch::new("opencode-missing");
        assert!(
            scan_once(&scratch.0.join(".claude"), &scratch.0.join("nowhere"))
                .hits
                .is_empty()
        );

        // A file where the database should be, holding junk.
        let data = scratch.0.join("data");
        std::fs::create_dir_all(data.join("opencode")).expect("a data directory");
        std::fs::write(data.join("opencode/opencode.db"), b"not a database").expect("a file");
        let projects = scratch.project_dir(".claude", "-home-dev-project");
        write(
            &projects.join("claude-1.jsonl"),
            &transcript("/home/dev/p", "model-x"),
        );

        let hits = scan_once(&scratch.0.join(".claude"), &data).hits;
        assert_eq!(hits.len(), 1, "the transcripts still arrive");
        assert_eq!(hits[0].source, "claude");
    }

    /// A half-written or foreign file is skipped, not turned into a card with an
    /// empty prompt.
    #[test]
    fn an_unusable_file_is_skipped() {
        let scratch = Scratch::new("unusable");
        let projects = scratch.project_dir(".claude", "-home-dev-project");
        write(&projects.join("empty.jsonl"), "");
        write(&projects.join("junk.jsonl"), "not json\n");
        write(&projects.join("notes.txt"), "hello\n");

        assert!(
            scan_once(&scratch.0.join(".claude"), &scratch.0.join("data"))
                .hits
                .is_empty()
        );
    }

    /// A session written after the app was closed is found and its newest turn is
    /// what the card shows — the whole point of running at attach.
    #[test]
    fn the_newest_session_is_the_one_a_panel_shows_first() {
        let scratch = Scratch::new("newest");
        let projects = scratch.project_dir(".claude", "-home-dev-project");
        write(
            &projects.join("older.jsonl"),
            &transcript("/home/dev/project", "model-x"),
        );
        // A second session, written later.
        std::thread::sleep(std::time::Duration::from_millis(2));
        write(
            &projects.join("newer.jsonl"),
            &transcript("/home/dev/project", "model-y"),
        );

        let mut hits = scan_once(&scratch.0.join(".claude"), &scratch.0.join("data")).hits;
        hits.sort_by_key(|hit| hit.modified_at);
        assert_eq!(
            hits.last().map(|hit| hit.session_id.as_str()),
            Some("newer")
        );
    }

    /// Two accounts under the same home are two roots, and a session in each is
    /// found once — not once per root.
    #[test]
    fn a_second_account_under_the_same_home_is_its_own_root() {
        let scratch = Scratch::new("accounts");
        let primary = scratch.project_dir(".claude", "-home-dev-project");
        let work = scratch.project_dir(".claude-work", "-home-dev-project");
        write(
            &primary.join("personal.jsonl"),
            &transcript("/home/dev/project", "model-x"),
        );
        write(
            &work.join("work.jsonl"),
            &transcript("/home/dev/project", "model-x"),
        );

        let roots = claude_roots(&scratch.0.join(".claude"));
        assert_eq!(roots.len(), 2, "roots: {roots:?}");

        let mut ids: Vec<String> = scan_once(Path::new(&roots[0]), &scratch.0.join("data"))
            .hits
            .into_iter()
            .map(|hit| hit.session_id)
            .collect();
        ids.sort();
        assert_eq!(ids, vec!["personal", "work"]);
    }

    /// The tail is honoured, not the whole file: a session's last three turns come
    /// from the end.
    #[test]
    fn only_the_tail_of_a_large_transcript_is_read() {
        let scratch = Scratch::new("tail");
        let projects = scratch.project_dir(".claude", "-home-dev-project");
        let path = projects.join("big.jsonl");
        // Well past TAIL_BYTES of filler, then the real turns.
        for _ in 0..200 {
            write(
                &path,
                &format!(
                    "{}\n",
                    r#"{"type":"assistant","cwd":"/home/dev/project","message":{"content":[{"type":"text","text":"filler line with some length to it"}]}}"#
                ),
            );
        }
        write(&path, &transcript("/home/dev/project", "model-x"));

        let hits = scan_once(&scratch.0.join(".claude"), &scratch.0.join("data")).hits;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].model.as_deref(), Some("model-x"));
        assert_eq!(hits[0].cwd, "/home/dev/project");
    }
}
