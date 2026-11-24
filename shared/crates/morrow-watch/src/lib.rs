//! "Something under these directories moved" — the event source discovery asks
//! instead of a timer (#286).
//!
//! # What this is and is not
//!
//! It is an OS edge, not policy: it never decides *when* to scan, only whether
//! anything changed since the last question. `morrow_core::scan_policy` decides
//! everything else, so a platform arm here cannot change what the shell does — only
//! how soon it hears.
//!
//! **None of the platform code has ever run.** The Linux arm compiles for
//! `x86_64-unknown-linux-gnu` and the Windows arm for `x86_64-pc-windows-msvc`, both
//! from a macOS machine; neither has watched a real directory. That is stated here
//! rather than discovered later: the fallback below is what this machine can
//! actually prove, and `Watcher::new` returning `Err` is a supported outcome the
//! shell already handles by falling back to the timer.
//!
//! # Why not "watch each store file"
//!
//! A store is a directory tree that grows: Claude Code mints a project directory per
//! cwd, Grok a session directory per session. Watching files would mean watching a
//! set that changes under you, so this watches the roots and lets a new directory be
//! a change like any other.

use std::path::{Path, PathBuf};
use std::time::Duration;

/// What asked-for directories to watch, and how to be told they moved.
pub struct Watcher {
    platform: platform::Platform,
}

impl Watcher {
    /// Watch `roots`. A root that does not exist yet is watched for its **parent**,
    /// so a store created after the shell started is noticed rather than missed.
    ///
    /// `Err` when the OS refuses, which the caller is expected to survive: the
    /// timer is a complete, if slower, answer.
    pub fn new(roots: &[PathBuf]) -> std::io::Result<Self> {
        let mut wanted: Vec<PathBuf> = Vec::new();
        for root in roots {
            if root.exists() {
                wanted.push(root.clone());
            } else if let Some(parent) = root.parent() {
                wanted.push(parent.to_path_buf());
            }
        }
        wanted.sort();
        wanted.dedup();
        Ok(Self {
            platform: platform::Platform::new(&wanted)?,
        })
    }

    /// Block until something under a watched root moves, or until `timeout`.
    ///
    /// `true` means "ask the policy again"; it does not mean "scan now". A caller
    /// that acts on this without the policy would scan on every keystroke an agent
    /// writes.
    pub fn wait(&mut self, timeout: Duration) -> bool {
        self.platform.wait(timeout)
    }

    /// The roots actually being watched, for a log line.
    pub fn roots(&self) -> &[PathBuf] {
        self.platform.roots()
    }
}

/// A store root, spelled the way the shell's readers spell it.
///
/// Kept here rather than in the shell so the list has one home; it is a list of
/// paths, not policy.
pub fn default_roots(home: &Path, config_dir: &Path) -> Vec<PathBuf> {
    vec![
        config_dir.join("projects"),
        home.join(".codex/sessions"),
        home.join(".local/share/opencode"),
        home.join(".cursor/projects"),
        home.join(".gemini/tmp"),
        home.join(".copilot/session-state"),
        home.join(".grok/sessions"),
        home.join(".kimi-code"),
        home.join(".kiro/sessions"),
        home.join(".qoder/projects"),
        home.join(".codebuddy/projects"),
        home.join(".factory/sessions"),
        home.join(".config/Code/User/globalStorage/saoudrizwan.claude-dev"),
    ]
}

#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod platform;

#[cfg(windows)]
#[path = "windows.rs"]
mod platform;

/// Everything else — including the machine this port is developed on — gets a
/// poll of the roots' modification times.
///
/// It is a real fallback, not a stub: it reports the same `bool`, so the shell's
/// behaviour does not change with the platform, only its latency.
#[cfg(not(any(target_os = "linux", windows)))]
#[path = "fallback.rs"]
mod platform;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_root_that_does_not_exist_yet_is_watched_through_its_parent() {
        let scratch = std::env::temp_dir().join(format!("morrow-watch-{}", std::process::id()));
        std::fs::create_dir_all(&scratch).expect("a scratch dir");
        let missing = scratch.join("not-yet");
        let watcher = Watcher::new(&[missing]).expect("a watcher");
        assert!(
            watcher.roots().contains(&scratch),
            "the parent is watched, so the store appearing is noticed"
        );
    }

    #[test]
    fn a_watcher_over_nothing_answers_nothing_rather_than_failing() {
        let mut watcher = Watcher::new(&[]).expect("a watcher");
        assert!(!watcher.wait(Duration::from_millis(10)));
    }

    #[test]
    fn the_default_roots_name_every_store_the_readers_read() {
        let roots = default_roots(Path::new("/home/dev"), Path::new("/home/dev/.claude"));
        // Spot-check the ones that are easy to get wrong: Claude's projects live
        // under the *config* dir, which the installer resolves (#258), not under
        // a hard-coded `~/.claude`.
        assert!(roots.contains(&PathBuf::from("/home/dev/.claude/projects")));
        assert!(roots.contains(&PathBuf::from("/home/dev/.codex/sessions")));
        assert!(roots.contains(&PathBuf::from(
            "/home/dev/.config/Code/User/globalStorage/saoudrizwan.claude-dev"
        )));
    }
}
