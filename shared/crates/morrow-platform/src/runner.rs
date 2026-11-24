//! Running the tools a desktop already ships.
//!
//! Nothing in `morrow-platform` links a D-Bus, X11 or audio library: it asks
//! `playerctl`, `wpctl`, `swaymsg`, `loginctl`, `notify-send` and friends, which
//! are the same tools the user could type. A missing tool is a capability the
//! UI states, not a crash — and the injected trait keeps every adapter testable
//! without a desktop behind it.

use std::path::Path;
use std::process::Command;
use std::io::Write;
use std::process::{Command, Stdio};

/// Whether `path` is a regular file the shell could actually spawn as a tool.
///
/// A file that is not executable must not make a capability row promise
/// something the feed cannot run. The Capability probe and the sound player
/// share this one answer so they cannot disagree about what is on `PATH`.
/// On Windows there is no Unix executable bit, so a regular file is enough —
/// the same rule the player applies there.
pub(crate) fn is_executable_tool(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    true
}

/// Runs a program and gives back its stdout. An error is the sentence the card
/// shows, not a panic.
pub trait Runner {
    fn run(&self, program: &str, args: &[&str]) -> Result<String, String>;

    /// Like [`Runner::run`], but feeds `stdin` to the child. The default ignores
    /// it so the existing adapters need no change; callers with a secret to keep
    /// out of argv (the quota bearer header) override this.
    fn run_with_stdin(&self, program: &str, args: &[&str], stdin: &str) -> Result<String, String> {
        let _ = stdin;
        self.run(program, args)
    }
}

/// The real thing.
pub struct SystemRunner;

impl Runner for SystemRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<String, String> {
        let output = Command::new(program)
            .args(args)
            .output()
            .map_err(|error| format!("{program}: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "{program} exited {}\n{}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    fn run_with_stdin(&self, program: &str, args: &[&str], stdin: &str) -> Result<String, String> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("{program}: {error}"))?;
        if let Some(mut input) = child.stdin.take() {
            input
                .write_all(stdin.as_bytes())
                .map_err(|error| format!("{program}: writing stdin: {error}"))?;
        }
        let output = child
            .wait_with_output()
            .map_err(|error| format!("{program}: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "{program} exited {}\n{}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::is_executable_tool;

    #[test]
    #[cfg(unix)]
    fn only_an_executable_regular_file_is_a_tool() {
        use std::os::unix::fs::PermissionsExt;
        use std::time::{SystemTime, UNIX_EPOCH};

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("a clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("morrow-tool-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let tool = dir.join("pw-play");

        std::fs::write(&tool, "#!/bin/sh\n").expect("write");
        assert!(
            !is_executable_tool(&tool),
            "a non-executable file is not a tool"
        );

        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        assert!(
            is_executable_tool(&tool),
            "the same file once it is executable"
        );

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }
}
