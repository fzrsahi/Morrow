//! The process table the session correlation reads (#266).
//!
//! `morrow_core::agent_process` decides *which* process is a session; this
//! supplies what it decides over. Kept apart so the decision is testable against
//! a fake table — the real one needs a machine running the agents.
//!
//! What each platform can answer is not the same, and the difference is real:
//!
//! | | executable | argv | cwd | start time |
//! |---|---|---|---|---|
//! | Linux (`/proc`) | yes | yes | yes | yes |
//! | Windows (Toolhelp) | name only | no | no | no |
//!
//! So on Windows a session is recognised by executable name and cannot be told
//! apart by working directory. That is a limitation, not an oversight: argv and
//! cwd live in another process's PEB, which needs `NtQueryInformationProcess` and
//! a `ReadProcessMemory` of structures that differ per build. The name is enough
//! for the common case — one `claude.exe` per card — and the honest alternative
//! (no pid at all) is worse.

use morrow_core::agent_process::ProcessInfo;

/// The table this host can build. The Linux reader is compiled **on every
/// host**, not behind `#[cfg(target_os = "linux")]`: a module that is skipped on
/// the machine we develop on is a module nobody type-checks, and cross-checking
/// Linux from macOS needs a Linux C toolchain that a SQLite dependency already
/// requires and does not have. Compiled everywhere, run where it is right.
pub fn table() -> Vec<ProcessInfo> {
    #[cfg(windows)]
    {
        toolhelp_table()
    }
    #[cfg(not(windows))]
    {
        // macOS has no `/proc`; the walk finds nothing and answers an empty table,
        // which is the state before #266 rather than a wrong one.
        proc_table()
    }
}

/// Split a `/proc/<pid>/cmdline`: NUL-separated, with a trailing NUL. Empty
/// fields are the padding, not arguments.
pub fn parse_cmdline(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).into_owned())
        .collect()
}

/// Rust cannot tell `argv[0]` from an argument here — `/proc` stores them in one
/// blob — so the whole vector is returned and the classifier reads it the way it
/// reads a real argv.
///
/// Compiled on every host (see `table`), used only off Windows: on the Windows
/// lane it is dead, which is the point of it being compiled there.
#[cfg_attr(windows, allow(dead_code))]
fn proc_table() -> Vec<ProcessInfo> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    let mut processes = Vec::new();
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<i32>().ok())
        else {
            continue;
        };
        // Only what can be read. A process owned by another user is a permission
        // error, not a reason to stop.
        let executable = std::fs::read_link(format!("/proc/{pid}/exe"))
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();
        if executable.is_empty() {
            continue;
        }
        let arguments = std::fs::read(format!("/proc/{pid}/cmdline"))
            .map(|bytes| parse_cmdline(&bytes))
            .unwrap_or_default();
        let cwd = std::fs::read_link(format!("/proc/{pid}/cwd"))
            .ok()
            .map(|path| path.to_string_lossy().into_owned());
        // Field 22 of `stat` is clock ticks since boot, not a timestamp; scaling it
        // as if it were milliseconds keeps the *ordering* the correlation needs,
        // which is all it compares.
        let start_time_ms = crate::process::start_time(i64::from(pid)).map(|ticks| ticks * 10);
        processes.push(ProcessInfo {
            pid,
            executable,
            arguments,
            cwd,
            start_time_ms,
        });
    }
    processes
}

/// Toolhelp gives every process's pid and executable name in one snapshot.
///
/// `arguments` and `cwd` are left empty rather than guessed: see this module's
/// note. The correlation treats an empty argv as "cannot read", which counts as
/// the CLI — a machine that will not tell us should not cost the user their
/// session.
#[cfg(windows)]
fn toolhelp_table() -> Vec<ProcessInfo> {
    let Ok((_parents, names)) = crate::windows::process::table() else {
        return Vec::new();
    };
    names
        .into_iter()
        .map(|(pid, name)| ProcessInfo {
            pid: i32::try_from(pid).unwrap_or_default(),
            executable: name,
            arguments: Vec::new(),
            cwd: None,
            start_time_ms: None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Linux reader's only non-trivial parsing, tested on any host.
    #[test]
    fn a_cmdline_is_nul_separated_with_a_trailing_nul() {
        assert_eq!(
            parse_cmdline(b"claude\0--resume\0"),
            vec!["claude".to_owned(), "--resume".to_owned()]
        );
        // No trailing NUL, which happens while a process is starting.
        assert_eq!(parse_cmdline(b"claude"), vec!["claude".to_owned()]);
        // An empty blob is no arguments, not one empty one.
        assert!(parse_cmdline(b"").is_empty());
        assert!(parse_cmdline(b"\0\0").is_empty());
        // An argument holding a space stays one argument.
        assert_eq!(
            parse_cmdline(b"claude\0-p\0run config\0"),
            vec![
                "claude".to_owned(),
                "-p".to_owned(),
                "run config".to_owned()
            ]
        );
        // Bytes that are not UTF-8 do not lose the whole argv.
        assert_eq!(parse_cmdline(b"claude\0\xff\0").len(), 2);
    }

    /// The table is never an error: a correlation with no candidates leaves the
    /// session without a pid, which is the state before #266.
    #[test]
    fn the_table_answers_a_list_on_any_host() {
        let table = table();
        // Whatever the host, every row is self-consistent.
        for info in &table {
            assert!(info.pid > 0);
        }
    }
}
