//! The OS half of the Workspace tab (#462). `git` is a process the desktop
//! already ships, so this module runs it through [`crate::runner::Runner`] and
//! hands the text to `morrow_core::git` parsers. Branch and worktree identity
//! still come from the pure `.git` reader, not a second process.

use std::path::Path;

use morrow_core::git::{self, Disk, DiskEntry, GitCommit, GitStatusCounts};
use serde::Serialize;

use crate::runner::Runner;

/// What the Workspace tab draws. Flattened counts mirror the core struct, so
/// the webview sees `staged`, `modified` and `untracked` at the top level.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStatus {
    pub cwd: String,
    pub project: String,
    pub branch: String,
    pub is_worktree: bool,
    #[serde(flatten)]
    pub counts: GitStatusCounts,
    pub commits: Vec<GitCommit>,
}

/// The same metadata read `morrow-core::git::read` expects: directories, and
/// files as text. Symlinks are not followed, as on the Swift side.
struct StdDisk;

impl Disk for StdDisk {
    fn entry(&self, path: &Path) -> Option<DiskEntry> {
        let metadata = std::fs::symlink_metadata(path).ok()?;
        if metadata.is_dir() {
            Some(DiskEntry::Directory)
        } else if metadata.is_file() {
            Some(DiskEntry::File(
                std::fs::read_to_string(path).unwrap_or_default(),
            ))
        } else {
            None
        }
    }
}

/// None means the cwd is empty or not inside a git checkout. An error means
/// git could not be run: the UI shows that, rather than an empty repository.
/// A repository with no commits still has a status, but no log rows.
pub fn status(cwd: &str, runner: &dyn Runner, limit: usize) -> Result<Option<WorkspaceStatus>, String> {
    if cwd.trim().is_empty() {
        return Ok(None);
    }
    let Some(info) = git::read(cwd, &StdDisk) else {
        return Ok(None);
    };
    let status = runner.run("git", &["-C", cwd, "status", "--porcelain=v1"])?;
    let format = "--pretty=tformat:%h%x1f%s";
    let max_count = format!("--max-count={limit}");
    let commits = match runner.run("git", &["-C", cwd, "log", &max_count, &format]) {
        Ok(log) => git::parse_log(&log, limit),
        // An unborn branch has no log to read; the status is still real.
        Err(_) => Vec::new(),
    };
    let project = Path::new(cwd)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| cwd.to_owned());
    Ok(Some(WorkspaceStatus {
        cwd: cwd.to_owned(),
        project,
        branch: info.branch,
        is_worktree: info.is_worktree,
        counts: git::parse_status(&status),
        commits,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Default)]
    struct FakeRunner {
        calls: Mutex<Vec<(String, Vec<String>)>>,
        answer: String,
        fail_log: bool,
    }

    impl Runner for FakeRunner {
        fn run(&self, program: &str, args: &[&str]) -> Result<String, String> {
            self.calls.lock().unwrap().push((
                program.to_owned(),
                args.iter().map(|arg| (*arg).to_owned()).collect(),
            ));
            if self.fail_log && args.iter().any(|arg| *arg == "log") {
                return Err("no commits yet".into());
            }
            Ok(self.answer.clone())
        }
    }

    fn checkout() -> (PathBuf, PathBuf) {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("a clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("morrow-workspace-{nanos}"));
        let git_dir = root.join(".git");
        std::fs::create_dir_all(&git_dir).expect("git dir");
        std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/feat/workspace\n").expect("HEAD");
        (root, git_dir)
    }

    #[test]
    fn status_uses_core_for_branch_and_parses_both_process_outputs() {
        let (cwd, _git_dir) = checkout();
        let runner = FakeRunner {
            answer: " M changed.rs\n?? new.rs\n".into(),
            ..FakeRunner::default()
        };
        let got = status(cwd.to_str().unwrap(), &runner, 5)
            .expect("git ran")
            .expect("a checkout");
        assert_eq!(got.branch, "feat/workspace");
        assert!(!got.is_worktree);
        assert_eq!(got.project, cwd.file_name().unwrap().to_str().unwrap());
        assert_eq!(got.counts.modified, 1);
        assert_eq!(got.counts.untracked, 1);
        assert_eq!(got.counts.staged, 0);
        let calls = runner.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, "git");
        assert_eq!(calls[0].1, ["-C", cwd.to_str().unwrap(), "status", "--porcelain=v1"]);
        assert!(calls[1].1.iter().any(|arg| arg == "log"));
        drop(calls);
        std::fs::remove_dir_all(&cwd).expect("cleanup");
    }

    #[test]
    fn log_failure_is_an_empty_recent_list_not_a_failed_tab() {
        let (cwd, _git_dir) = checkout();
        let runner = FakeRunner {
            answer: "A  added.rs\n".into(),
            fail_log: true,
            ..FakeRunner::default()
        };
        let got = status(cwd.to_str().unwrap(), &runner, 5)
            .expect("git ran")
            .expect("a checkout");
        assert_eq!(got.counts.staged, 1);
        assert!(got.commits.is_empty());
        std::fs::remove_dir_all(&cwd).expect("cleanup");
    }

    #[test]
    fn empty_cwd_and_non_repo_return_none_without_spawning_git() {
        let runner = FakeRunner::default();
        assert!(status("", &runner, 5).expect("no error").is_none());
        let missing = std::env::temp_dir().join("morrow-not-a-checkout");
        assert!(status(missing.to_str().unwrap(), &runner, 5)
            .expect("no error")
            .is_none());
        assert!(runner.calls.lock().unwrap().is_empty());
    }
}
