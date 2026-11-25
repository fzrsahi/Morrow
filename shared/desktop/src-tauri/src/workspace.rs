//! Workspace tab command (#462). The shell owns the process and the core owns
//! the parsing; this module is only the IPC edge. Nothing polls unless the tab
//! is open, so a closed island runs no git scans.

use morrow_platform::git::WorkspaceStatus;
use morrow_platform::runner::SystemRunner;

/// The committed history the tab shows. Five rows match macOS's recent list.
const COMMIT_LIMIT: usize = 5;

/// `null` when there is no session cwd or it is not a git checkout; an error
/// only when `git` itself cannot be run.
#[tauri::command]
pub fn workspace_status(cwd: Option<String>) -> Result<Option<WorkspaceStatus>, String> {
    morrow_platform::git::status(cwd.as_deref().unwrap_or_default(), &SystemRunner, COMMIT_LIMIT)
}
