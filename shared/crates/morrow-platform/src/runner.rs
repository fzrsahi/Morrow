//! Running the tools a desktop already ships.
//!
//! Nothing in `morrow-platform` links a D-Bus, X11 or audio library: it asks
//! `playerctl`, `wpctl`, `swaymsg`, `loginctl`, `notify-send` and friends, which
//! are the same tools the user could type. A missing tool is a capability the
//! UI states, not a crash — and the injected trait keeps every adapter testable
//! without a desktop behind it.

use std::io::Write;
use std::process::{Command, Stdio};

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
