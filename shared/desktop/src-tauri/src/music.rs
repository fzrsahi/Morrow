//! The music card's commands (#102): what is playing, the controls, the volume
//! and the lyrics. The readings are `morrow_platform::music`; this only reads
//! the clock and hands the results to the card.
//!
//! No polling here: the card asks when it is open, which is the only time a
//! music card is on screen.

#[cfg(unix)]
use std::sync::OnceLock;
#[cfg(unix)]
use std::time::Instant;

use morrow_core::music::{LyricLine, Track};
// lrclib and the `Runner` are the same on both platforms; only the player and
// the volume differ (#107).
use morrow_platform::music::{self, SystemRunner};
#[cfg(unix)]
use morrow_platform::music::{NowPlaying, SystemVolume};

static RUNNER: SystemRunner = SystemRunner;

/// Unix-only since #239 AC 1: its one Windows caller was `music_position`, an IPC
/// command with no caller. Removing it left this unused on Windows, which
/// `windows/ci/check.sh` (`-D warnings`) said out loud.
#[cfg(unix)]
fn monotonic() -> i64 {
    static START: OnceLock<Instant> = OnceLock::new();
    let start = START.get_or_init(Instant::now);
    start.elapsed().as_millis() as i64
}

/// The current track, or `null` when nothing is playing. SMTC on Windows, MPRIS
/// through `playerctl` everywhere else (#107).
#[tauri::command]
pub fn music_track() -> Result<Option<Track>, String> {
    #[cfg(windows)]
    return morrow_platform::windows::media::track();
    #[cfg(unix)]
    return NowPlaying::new(&RUNNER, monotonic()).track();
}

#[tauri::command]
pub fn music_play_pause() -> Result<(), String> {
    #[cfg(windows)]
    return morrow_platform::windows::media::play_pause();
    #[cfg(unix)]
    return NowPlaying::new(&RUNNER, monotonic()).play_pause();
}

#[tauri::command]
pub fn music_next() -> Result<(), String> {
    #[cfg(windows)]
    return morrow_platform::windows::media::next();
    #[cfg(unix)]
    return NowPlaying::new(&RUNNER, monotonic()).next();
}

#[tauri::command]
pub fn music_previous() -> Result<(), String> {
    #[cfg(windows)]
    return morrow_platform::windows::media::previous();
    #[cfg(unix)]
    return NowPlaying::new(&RUNNER, monotonic()).previous();
}

/// Seek by an offset in microseconds (the ±10 s buttons).
#[tauri::command]
pub fn music_seek(offset_us: i64) -> Result<(), String> {
    #[cfg(windows)]
    return morrow_platform::windows::media::seek_by(offset_us).map(|_| ());
    #[cfg(unix)]
    return NowPlaying::new(&RUNNER, monotonic()).seek_by(offset_us);
}

/// The system volume.
#[tauri::command]
pub fn music_volume() -> Result<morrow_core::music::Volume, String> {
    #[cfg(windows)]
    return morrow_platform::windows::volume::read();
    #[cfg(unix)]
    return SystemVolume::new(&RUNNER).read();
}

#[tauri::command]
pub fn music_volume_set(percent: u8) -> Result<(), String> {
    #[cfg(windows)]
    return morrow_platform::windows::volume::set_percent(percent);
    #[cfg(unix)]
    return SystemVolume::new(&RUNNER).set_percent(percent);
}

#[tauri::command]
pub fn music_volume_mute() -> Result<(), String> {
    #[cfg(windows)]
    return morrow_platform::windows::volume::toggle_mute().map(|_| ());
    #[cfg(unix)]
    return SystemVolume::new(&RUNNER).toggle_mute();
}

/// Synced lyrics for a track, or `null` when the service has none.
#[tauri::command]
pub fn music_lyrics(track: Track) -> Result<Option<Vec<LyricLine>>, String> {
    music::lyrics(&RUNNER, &track)
}
