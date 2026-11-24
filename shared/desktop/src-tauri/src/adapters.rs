//! #103's commands: jump to a terminal, notice a session that ended, go quiet
//! while the screen is locked, and post a reminder.
//!
//! Each one is a thin call into `morrow_platform`; the policy lives there.

#[cfg(unix)]
use std::path::PathBuf;

use morrow_platform::process::Watch;
use morrow_platform::terminal::Outcome;
#[cfg(unix)]
use morrow_platform::terminal::Target;
#[cfg(unix)]
use morrow_platform::{lock, notify};
use tauri::State;

use crate::HooksSite;

/// Jump to a session's terminal, or hand over its path.
#[tauri::command]
pub fn terminal_activate(
    site: State<'_, HooksSite>,
    app: Option<String>,
    tty: Option<String>,
    cwd: Option<String>,
    pid: Option<i64>,
) -> Outcome {
    let _ = &site;
    // Windows has no app-id protocol to focus by: the window is found through
    // the session's process chain, so that path names the pid instead (#187).
    #[cfg(windows)]
    {
        let _ = app;
        let Some(pid) = pid.and_then(|pid| u32::try_from(pid).ok()) else {
            return Outcome::NotPossible("this session has no local process to follow".to_owned());
        };
        let (parents, names) = match morrow_platform::windows::process::table() {
            Ok(table) => table,
            Err(reason) => return Outcome::NotPossible(reason),
        };
        match morrow_platform::windows::terminal::activate(&parents, pid, &names) {
            Ok(()) => Outcome::Focused,
            Err(reason) => {
                // L-009's floor: a click that cannot raise a window still hands
                // over the working directory instead of doing nothing.
                if let Some(text) = cwd.as_deref().or(tty.as_deref()) {
                    match morrow_platform::windows::clipboard::copy(text) {
                        Ok(()) => Outcome::Copied(text.to_owned()),
                        Err(failure) => Outcome::NotPossible(format!("{reason}; {failure}")),
                    }
                } else {
                    Outcome::NotPossible(reason)
                }
            }
        }
    }
    #[cfg(unix)]
    {
        let _ = pid;
        let target = Target {
            app,
            tty,
            cwd: cwd.map(PathBuf::from),
        };
        // X11/Wayland focus a window, never a pane; `terminal::activate` tries
        // the compositor's window API before the clipboard fallback.
        morrow_platform::terminal::activate(&morrow_platform::runner::SystemRunner, &target)
    }
}

/// Remember a session's process so a later call can tell whether it is the same
/// one. `None` when the pid is not running.
#[tauri::command]
pub fn process_watch(pid: i64) -> Option<Watch> {
    Watch::new(pid)
}

/// Whether the process behind a watch is still there.
#[tauri::command]
pub fn process_alive(watch: Watch) -> bool {
    watch.alive()
}

/// Whether the screen is locked. `None` when the session cannot say, which the
/// island treats as unlocked.
#[tauri::command]
pub fn screen_locked() -> Option<bool> {
    #[cfg(windows)]
    return morrow_platform::windows::lock::locked();
    #[cfg(unix)]
    return lock::locked(&morrow_platform::runner::SystemRunner);
}

/// Post a reminder through the desktop's notification service.
#[tauri::command]
pub fn notify_send(title: String, body: String) -> Result<(), String> {
    #[cfg(windows)]
    return morrow_platform::windows::toast::notify(&title, &body);
    #[cfg(unix)]
    return notify::notify(&morrow_platform::runner::SystemRunner, &title, &body);
}

/// Whether a fullscreen app is in front, so the island can step around it.
#[tauri::command]
pub fn fullscreen() -> bool {
    #[cfg(windows)]
    return morrow_platform::windows::fullscreen::fullscreen();
    #[cfg(unix)]
    return false;
}

/// Whether Windows' quiet time is active: the same notification state that
/// answers `fullscreen`, read for the moon in the collapsed bar (#197). There
/// is no quiet-hours clock in the Linux shell yet, so it stays false there.
#[tauri::command]
pub fn quiet_hours() -> bool {
    #[cfg(windows)]
    return morrow_platform::windows::fullscreen::state()
        == morrow_platform::windows_rules::NotificationState::QuietTime;
    #[cfg(unix)]
    return false;
}

/// The pointer, in screen pixels.
#[tauri::command]
pub fn pointer_position() -> Option<(i32, i32)> {
    #[cfg(windows)]
    return morrow_platform::windows::pointer::position();
    #[cfg(unix)]
    return None;
}
