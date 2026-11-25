//! #465 — the Buddy page's frame preview. The BLE transport stays out of scope
//! (no reader, no pairing, #240); this command makes the already-ported wire
//! format reachable from the shell so the Settings page can show the bytes a
//! real device would receive for the chosen settings.

use morrow_core::buddy::{
    encode_brightness, encode_orientation, MascotFrame, MascotId, MascotStatusCode, ScreenOrientation,
};
use morrow_core::reduce::AgentStatus;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02X}")).collect::<Vec<_>>().join(" ")
}

/// The three frames a screen would get: the mascot's state, its brightness and
/// which way up it is. Unknown sources fall back to Claude, as the island does.
pub fn preview_frames(source: &str, status: &str, brightness: f64, orientation: &str) -> Vec<String> {
    let mascot = MascotId::from_source(Some(source)).unwrap_or(MascotId::Claude);
    let agent_status = match status {
        "idle" => AgentStatus::Idle,
        "running" => AgentStatus::Running,
        "waitingApproval" => AgentStatus::WaitingApproval,
        "waitingQuestion" => AgentStatus::WaitingQuestion,
        _ => AgentStatus::Processing,
    };
    let state = MascotFrame {
        mascot,
        status: MascotStatusCode::from_status(agent_status),
        tool_name: None,
    };
    vec![
        hex(&state.encode()),
        hex(&encode_brightness(brightness as u8)),
        hex(&encode_orientation(ScreenOrientation::from_settings(Some(orientation)))),
    ]
}

#[tauri::command]
pub fn buddy_frame_preview(
    source: String,
    status: String,
    brightness: f64,
    orientation: String,
) -> Vec<String> {
    preview_frames(&source, &status, brightness, &orientation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_preview_shows_the_three_wire_frames() {
        let frames = preview_frames("codex", "idle", 0.0, "down");
        assert_eq!(frames[0], "01 00 00");
        assert_eq!(frames[1], "FE 0A"); // brightness clamped to 10
        assert_eq!(frames[2], "FD 01");
    }

    #[test]
    fn an_unknown_source_falls_back_to_claude() {
        assert_eq!(preview_frames("nobody", "idle", 70.0, "up")[0], "00 00 00");
    }
}
