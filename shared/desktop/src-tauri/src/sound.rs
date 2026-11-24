//! #241: the sound effects, for Settings.
//!
//! The island plays its cues on its own (see `feed`). These two commands exist
//! so the Sound page can list the table and let the user hear a row. A preview
//! is a direct action, so it plays even on a locked screen — the same rule
//! macOS keeps for `preview`/`previewCustom`, which deliberately stay out of
//! the event path.

use morrow_platform::sound;

/// One row of the Sound page: the cue's name and the file that plays it.
#[derive(serde::Serialize)]
pub struct CueRow {
    pub name: &'static str,
    pub file: &'static str,
}

#[tauri::command]
pub fn sound_cues() -> Vec<CueRow> {
    sound::CUES
        .iter()
        .map(|cue| CueRow {
            name: cue.name,
            file: cue.file,
        })
        .collect()
}

/// Play one cue now, for the page's play buttons.
#[tauri::command]
pub fn sound_preview(cue: String) -> Result<(), String> {
    match sound::player_on_path() {
        Some(player) => sound::play(player, &cue),
        None => Err("no sound player on this machine".to_owned()),
    }
}
