//! Which local process is which agent session (#266).
//!
//! `cli_pid` otherwise only comes from a hook's `_ppid`, so a session whose hook
//! did not carry one is never pruned and a backfilled session (#261) has no pid at
//! all. This is the other source: the process table.
//!
//! ## What is ported, and what is not
//!
//! `AgentProcessFinder.swift` is 796 lines, and most of it is macOS bundle paths
//! — `/cursor.app/contents/`, `/claude.app/contents/`, thirty per-source tables
//! of them. There are no `.app` bundles on Linux or Windows, so that half has no
//! port to make.
//!
//! What does port is the half that decides *whether a process is a session at
//! all*, and it is not macOS-specific: Claude Code's binary is multicall, so the
//! same executable serves as `ugrep`, `bfs`, `rg` and `apply-seccomp` for its own
//! tools, as detached background hosts, and as MCP servers. Count any of those as
//! the CLI and a user with one running session gets a handful of phantom cards
//! that never die.

/// One process, as a platform's table reports it.
///
/// `arguments` and `cwd` are optional because not every platform can answer them
/// cheaply: a Windows Toolhelp snapshot gives the executable name and nothing
/// else without reading another process's PEB.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: i32,
    pub executable: String,
    pub arguments: Vec<String>,
    pub cwd: Option<String>,
    /// Milliseconds since the Unix epoch, so pid reuse can be told apart.
    pub start_time_ms: Option<u64>,
}

/// argv[0] names Claude Code's native binary running as a **tool**, not as Claude
/// Code. The shell it gives its Bash tool defines `grep`, `find` and `rg` as
/// `exec -a ugrep|bfs|rg <binary>`, and its Grep tool starts `rg` the same way.
/// `apply-seccomp` is its Linux sandbox helper.
pub const CLAUDE_CODE_TOOL_NAMES: &[&str] = &["ugrep", "bfs", "rg", "apply-seccomp"];

/// Arguments that start the binary as Claude Code's own machinery rather than a
/// session: the detached terminal host and the spare worker inside it, daemon
/// workers, preloading, the `gh` shim, and the MCP servers and browser native
/// host it serves. No user passes them.
pub const CLAUDE_CODE_HELPER_FLAGS: &[&str] = &[
    "--bg-pty-host",
    "--bg-spare",
    "--daemon-worker",
    "--preload",
    "--gh-standin",
    "--chrome-native-host",
    "--claude-in-chrome-mcp",
    "--computer-use-mcp",
    "--eval-mock-server",
];

/// Subcommands that run no session: `claude mcp serve` is an MCP server for
/// another client, and the rest change settings or the install and exit.
pub const CLAUDE_CODE_NON_SESSION_SUBCOMMANDS: &[&str] = &[
    "mcp",
    "config",
    "doctor",
    "update",
    "install",
    "uninstall",
    "setup-token",
    "plugin",
    "auth",
];

/// Whether argv starts Claude Code's CLI — interactive, `-p`, `--resume`,
/// `--continue` — rather than a tool, a background helper or a settings
/// subcommand that runs the same binary.
///
/// An argv that cannot be read counts as the CLI: a machine that will not tell us
/// should not cost the user their session.
pub fn is_claude_code_cli_invocation(arguments: &[String]) -> bool {
    let Some(argv0) = arguments.first() else {
        return true;
    };
    // Background roles name themselves in argv[0]: `claude bg-pty-host`.
    if argv0.starts_with("claude ") {
        return false;
    }
    let last_component = argv0.rsplit(['/', '\\']).next().unwrap_or(argv0.as_str());
    if CLAUDE_CODE_TOOL_NAMES.contains(&last_component) {
        return false;
    }
    let rest = &arguments[1..];
    if rest
        .iter()
        .any(|argument| CLAUDE_CODE_HELPER_FLAGS.contains(&argument.as_str()))
    {
        return false;
    }
    if let Some(first) = rest.first() {
        if CLAUDE_CODE_NON_SESSION_SUBCOMMANDS.contains(&first.as_str()) {
            return false;
        }
    }
    true
}

/// The last path component of an executable, lowercased, without a Windows
/// `.exe`: what a name match compares.
pub fn executable_name(path: &str) -> String {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let name = name.to_ascii_lowercase();
    name.strip_suffix(".exe").map(str::to_owned).unwrap_or(name)
}

/// Whether `info` looks like a running session of `source`.
///
/// Only sources this port can actually recognise are listed. An unknown source
/// answers `false` — a wrong match puts a stranger's pid on a card and keeps it
/// alive until that process exits, which is worse than no pid.
pub fn is_session_process(source: &str, info: &ProcessInfo) -> bool {
    match source {
        // Claude Code is the one with a multicall binary, so the name alone is
        // not enough: `rg` and `claude` are the same executable.
        "claude" => {
            let name = executable_name(&info.executable);
            let looks_right = name == "claude" || name.starts_with("claude");
            looks_right
                && (info.arguments.is_empty() || is_claude_code_cli_invocation(&info.arguments))
        }
        // The rest are ordinary executables: the name is the whole test.
        "codex" => executable_name(&info.executable) == "codex",
        "gemini" => executable_name(&info.executable) == "gemini",
        "opencode" => executable_name(&info.executable) == "opencode",
        "grok" => executable_name(&info.executable) == "grok",
        "kimi" => executable_name(&info.executable) == "kimi",
        "qwen" => executable_name(&info.executable) == "qwen",
        // Qoder's China build ships the same binary under `qoderclicn`.
        "qoder" => matches!(
            executable_name(&info.executable).as_str(),
            "qoder" | "qodercli" | "qoderclicn"
        ),
        "codebuddy" => matches!(
            executable_name(&info.executable).as_str(),
            "codebuddy" | "codebuddy-cli"
        ),
        // Factory's CLI is `droid`; the store directory is `~/.factory`, which is
        // why the source and the executable do not share a name.
        "droid" => executable_name(&info.executable) == "droid",

        // The CLI binary is `cursor-agent`; `cursor` is the name it is sometimes
        // installed under. The IDE's own process is a desktop app, which this port
        // does not try to recognise (no `.app` bundles on Linux or Windows).
        "cursor" | "cursor-cli" | "cursor-agent" => {
            matches!(
                executable_name(&info.executable).as_str(),
                "cursor-agent" | "cursor"
            )
        }
        _ => false,
    }
}

/// Every live process of `source` whose working directory is `cwd`, oldest first.
///
/// `alive` is injected: a table read a moment ago can name a process that has
/// already exited, and a dead pid on a card is a card that never goes away.
/// Oldest first is deliberate — the oldest process in a directory is the one that
/// started the session, and a newer one is more likely a subagent or a retry.
pub fn pids_for_cwd<'a>(
    table: &'a [ProcessInfo],
    source: &str,
    cwd: &str,
    alive: impl Fn(i32) -> bool,
) -> Vec<&'a ProcessInfo> {
    let mut found: Vec<&ProcessInfo> = table
        .iter()
        .filter(|info| info.cwd.as_deref() == Some(cwd))
        .filter(|info| is_session_process(source, info))
        .filter(|info| alive(info.pid))
        .collect();
    found.sort_by(|a, b| {
        a.start_time_ms
            .unwrap_or(u64::MAX)
            .cmp(&b.start_time_ms.unwrap_or(u64::MAX))
            .then_with(|| a.pid.cmp(&b.pid))
    });
    found
}

/// One session that needs a pid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PidRequest {
    pub session_id: String,
    pub source: String,
    pub cwd: String,
    /// When the session's store was last written. A process that started *after*
    /// that cannot be the one that wrote it.
    pub modified_at_ms: Option<u64>,
}

/// A session and the process that is running it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assignment {
    pub session_id: String,
    pub pid: i32,
    pub start_time_ms: Option<u64>,
}

/// How far before a store's last write a process may have started and still have
/// written it. A CLI writes its first row just after it is running, and clock
/// granularity is not free.
pub const START_SLACK_MS: u64 = 10_000;

/// Give each request the pid that best explains it, never the same pid twice.
///
/// Two sessions in one directory is the case that goes wrong quietly: if both are
/// given the first pid, one card is watched by the other's process, so it outlives
/// its own and disappears when its neighbour exits. `claimed` carries the pids
/// already taken — by a hook that named one, or by an earlier session in this same
/// pass — and is updated as pids are handed out.
///
/// A process that started long after the store was last written is not a
/// candidate for it: that is how a second session in the same directory, started
/// later, is told apart from the one that wrote the file.
pub fn assign(
    table: &[ProcessInfo],
    requests: &[PidRequest],
    claimed: &mut std::collections::HashSet<i32>,
    alive: impl Fn(i32) -> bool,
) -> Vec<Assignment> {
    let mut assignments: Vec<Assignment> = Vec::new();
    // One table, reused: the candidates per (source, cwd) are computed on demand
    // but never by walking the table a second time per request.
    for request in requests {
        let candidates: Vec<&ProcessInfo> =
            pids_for_cwd(table, &request.source, &request.cwd, &alive)
                .into_iter()
                .filter(|info| !claimed.contains(&info.pid))
                .filter(|info| match (request.modified_at_ms, info.start_time_ms) {
                    (Some(modified), Some(started)) => started <= modified + START_SLACK_MS,
                    // A process whose start time is unknown is still a candidate:
                    // silence is not evidence against it.
                    _ => true,
                })
                .collect();
        // The process that best explains the store: the latest one that started
        // **at or before** the store was last written. A process that started
        // after that write did not make it — with two sessions in one directory
        // this is the whole difference between the right card and a card watched
        // by its neighbour's process.
        //
        // `candidates` is oldest-first. The slack in the filter above is for
        // clock granularity, not licence to pick a later process.
        let best = match request.modified_at_ms {
            Some(modified) => candidates
                .iter()
                .rfind(|info| info.start_time_ms.is_none_or(|started| started <= modified))
                // Nothing started before it: fall back to the earliest, which is
                // still better than no pid — the session is running somewhere.
                .or_else(|| candidates.first()),
            // No mtime to compare against: the newest is the best guess.
            None => candidates.last(),
        };
        let Some(best) = best else {
            continue;
        };
        claimed.insert(best.pid);
        assignments.push(Assignment {
            session_id: request.session_id.clone(),
            pid: best.pid,
            start_time_ms: best.start_time_ms,
        });
    }
    assignments
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn process(
        pid: i32,
        executable: &str,
        args: &[&str],
        cwd: Option<&str>,
        started: u64,
    ) -> ProcessInfo {
        ProcessInfo {
            pid,
            executable: executable.to_owned(),
            arguments: args.iter().map(|arg| (*arg).to_owned()).collect(),
            cwd: cwd.map(str::to_owned),
            start_time_ms: Some(started),
        }
    }

    fn argv(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_owned()).collect()
    }

    /// The four roles the multicall binary takes. Counting any of the three
    /// non-session ones as a session is a phantom card that never dies.
    #[test]
    fn the_multicall_binary_is_a_session_only_as_the_cli() {
        // The CLI, in the shapes a user starts it in.
        assert!(is_claude_code_cli_invocation(&argv(&["claude"])));
        assert!(is_claude_code_cli_invocation(&argv(&[
            "/home/u/.local/bin/claude",
            "-p",
            "do a thing"
        ])));
        assert!(is_claude_code_cli_invocation(&argv(&[
            "claude", "--resume"
        ])));
        // An argv that cannot be read counts as the CLI.
        assert!(is_claude_code_cli_invocation(&[]));

        // Tools: the same binary, exec'd under another name.
        for tool in ["ugrep", "bfs", "rg", "apply-seccomp"] {
            assert!(
                !is_claude_code_cli_invocation(&argv(&[tool])),
                "{tool} is a tool, not a session"
            );
        }

        // Background roles name themselves in argv[0].
        assert!(!is_claude_code_cli_invocation(&argv(&[
            "claude bg-pty-host"
        ])));

        // Helper flags.
        for flag in [
            "--bg-spare",
            "--daemon-worker",
            "--preload",
            "--gh-standin",
            "--chrome-native-host",
            "--claude-in-chrome-mcp",
            "--computer-use-mcp",
        ] {
            assert!(
                !is_claude_code_cli_invocation(&argv(&["claude", flag])),
                "{flag} is machinery, not a session"
            );
        }

        // Subcommands that run no session.
        for subcommand in ["mcp", "config", "doctor", "update", "auth"] {
            assert!(
                !is_claude_code_cli_invocation(&argv(&["claude", subcommand])),
                "`claude {subcommand}` runs no session"
            );
        }
        // …but the same words are fine as arguments to a session.
        assert!(is_claude_code_cli_invocation(&argv(&[
            "claude",
            "-p",
            "run config"
        ])));
    }

    #[test]
    fn an_executable_name_drops_the_directory_and_the_extension() {
        assert_eq!(executable_name("/usr/bin/claude"), "claude");
        assert_eq!(executable_name(r"C:\Users\u\AppData\claude.exe"), "claude");
        assert_eq!(executable_name("claude"), "claude");
        assert_eq!(executable_name("/opt/Codex.EXE"), "codex");
        assert_eq!(executable_name(""), "");
    }

    #[test]
    fn only_the_sources_this_port_recognises_answer_true() {
        let claude = process(1, "/usr/bin/claude", &["claude"], Some("/w"), 1);
        assert!(is_session_process("claude", &claude));
        // The same binary as a tool is not a session.
        let rg = process(2, "/usr/bin/claude", &["rg"], Some("/w"), 1);
        assert!(!is_session_process("claude", &rg));

        let codex = process(3, "/usr/bin/codex", &["codex"], Some("/w"), 1);
        assert!(is_session_process("codex", &codex));
        // A source with no rule never matches, rather than matching everything.
        assert!(!is_session_process("unknown-agent", &codex));
        assert!(!is_session_process("qoder", &codex));
    }

    /// A dead pid on a card is a card that never goes away.
    #[test]
    fn a_process_that_has_exited_is_not_a_candidate() {
        let table = vec![
            process(1, "/usr/bin/claude", &["claude"], Some("/w"), 1_000),
            process(2, "/usr/bin/claude", &["claude"], Some("/w"), 2_000),
        ];
        let alive = |pid: i32| pid == 2;
        let found = pids_for_cwd(&table, "claude", "/w", alive);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].pid, 2);
    }

    #[test]
    fn a_process_in_another_directory_is_not_a_candidate() {
        let table = vec![
            process(1, "/usr/bin/claude", &["claude"], Some("/w"), 1_000),
            process(2, "/usr/bin/claude", &["claude"], Some("/other"), 1_000),
            // Unknown cwd is not a match either.
            process(3, "/usr/bin/claude", &["claude"], None, 1_000),
        ];
        let found = pids_for_cwd(&table, "claude", "/w", |_| true);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].pid, 1);
    }

    /// The case that goes wrong quietly: two sessions in one directory.
    #[test]
    fn two_sessions_in_one_directory_get_different_pids() {
        let table = vec![
            process(10, "/usr/bin/claude", &["claude"], Some("/w"), 1_000),
            process(11, "/usr/bin/claude", &["claude"], Some("/w"), 5_000),
        ];
        let requests = vec![
            PidRequest {
                session_id: "a".into(),
                source: "claude".into(),
                cwd: "/w".into(),
                modified_at_ms: Some(1_500),
            },
            PidRequest {
                session_id: "b".into(),
                source: "claude".into(),
                cwd: "/w".into(),
                modified_at_ms: Some(5_500),
            },
        ];
        let mut claimed = HashSet::new();
        let assignments = assign(&table, &requests, &mut claimed, |_| true);

        assert_eq!(assignments.len(), 2);
        let pids: Vec<i32> = assignments.iter().map(|a| a.pid).collect();
        assert_eq!(
            pids,
            vec![10, 11],
            "each session gets the process that could have written it"
        );
        assert_eq!(claimed.len(), 2);
    }

    /// A pid a hook already named is not handed to a discovered session.
    #[test]
    fn a_pid_a_hook_already_named_is_not_reused() {
        let table = vec![process(
            10,
            "/usr/bin/claude",
            &["claude"],
            Some("/w"),
            1_000,
        )];
        let mut claimed: HashSet<i32> = [10].into_iter().collect();
        let requests = vec![PidRequest {
            session_id: "b".into(),
            source: "claude".into(),
            cwd: "/w".into(),
            modified_at_ms: Some(1_500),
        }];
        assert!(assign(&table, &requests, &mut claimed, |_| true).is_empty());
    }

    /// A process that started long after the store was written did not write it.
    #[test]
    fn a_process_that_started_after_the_store_was_written_is_not_a_candidate() {
        let table = vec![process(
            10,
            "/usr/bin/claude",
            &["claude"],
            Some("/w"),
            100_000,
        )];
        let requests = vec![PidRequest {
            session_id: "a".into(),
            source: "claude".into(),
            cwd: "/w".into(),
            modified_at_ms: Some(1_000),
        }];
        let mut claimed = HashSet::new();
        assert!(assign(&table, &requests, &mut claimed, |_| true).is_empty());

        // Just after the store was written, with the slack, it is one.
        let table = vec![process(
            10,
            "/usr/bin/claude",
            &["claude"],
            Some("/w"),
            1_000 + START_SLACK_MS,
        )];
        assert_eq!(assign(&table, &requests, &mut claimed, |_| true).len(), 1);
    }

    /// "Measured, not assumed": one pass over the table per assignment, not one
    /// per session. The counter is the measurement — a timing test would be a
    /// flake, and the cost being guarded is the *number* of walks.
    #[test]
    fn the_table_is_walked_once_per_session_not_once_per_process() {
        use std::cell::Cell;

        let table: Vec<ProcessInfo> = (0..500)
            .map(|index| {
                process(
                    index,
                    "/usr/bin/claude",
                    &["claude"],
                    Some(if index % 2 == 0 { "/a" } else { "/b" }),
                    1_000 + index as u64,
                )
            })
            .collect();
        let requests = vec![PidRequest {
            session_id: "a".into(),
            source: "claude".into(),
            cwd: "/a".into(),
            modified_at_ms: Some(10_000),
        }];

        let alive_calls = Cell::new(0usize);
        let assignments = assign(&table, &requests, &mut HashSet::new(), |_| {
            alive_calls.set(alive_calls.get() + 1);
            true
        });

        assert_eq!(assignments.len(), 1);
        // The liveness probe ran for the 250 entries in `/a`, once each, and not
        // for `/b` — a per-process-per-session walk would be 500 × sessions.
        assert_eq!(alive_calls.get(), 250);
    }
}
