//! Sound effects on Windows (#241): `PlaySoundW`, asynchronous.
//!
//! No tool to find and no bus to have — the player is part of the OS, which is
//! why `capability::Feature::Audio` is `Supported` here whatever `PATH` holds.

use std::path::Path;

use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_FILENAME, SND_NODEFAULT};
use windows::core::HSTRING;

/// Play a file the OS can open. `SND_ASYNC` returns before the sound ends, so
/// the feed is never held; `SND_NODEFAULT` keeps a missing file silent instead
/// of falling back to the system beep. One channel per process: a second call
/// replaces the first, which is what a turn boundary wants.
pub fn play(path: &Path) -> Result<(), String> {
    let wide = HSTRING::from(path.to_string_lossy().as_ref());
    // SAFETY: `wide` is a NUL-terminated UTF-16 buffer that outlives the call,
    // and `SND_FILENAME` means no module handle is read.
    let played = unsafe { PlaySoundW(&wide, None, SND_FILENAME | SND_ASYNC | SND_NODEFAULT) };
    if played.as_bool() {
        Ok(())
    } else {
        Err(format!("PlaySoundW could not play {}", path.display()))
    }
}
