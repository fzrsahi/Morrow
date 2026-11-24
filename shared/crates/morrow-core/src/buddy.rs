//! The Buddy (ESP32) wire format (`macos/Sources/MorrowCore/ESP32Protocol.swift`).
//!
//! Everything the little screen receives is one short frame on a BLE characteristic:
//! a marker byte, then the fields that frame carries. There is no transport here and
//! there will not be one until #238 AC 1 decides the `Ble` capability — this is the
//! part that can be checked without hardware, because the bytes either match the
//! firmware's table or they do not.
//!
//! Two things worth knowing before reading the constants, because both have silently
//! broken a real device:
//!
//! - **`0xE0`–`0xEF` is reserved for pairing on both directions.** A mascot id is
//!   0..15, so an uplink byte is read as a pair response first, then a mascot focus,
//!   then a control command. If the id range ever grows past `0xDF` that order has to
//!   be revisited, which is why the check lives in one place.
//! - **Short frames are the compatibility story.** Brightness and orientation are two
//!   bytes so that older firmware treats them as an ignored short payload instead of
//!   misreading a leading byte as an agent status.
//!
//! Strings are UTF-8 and **byte**-truncated, which may split a code point — the firmware
//! only uses those bytes for a marquee label, so a broken glyph is cheaper than a frame
//! over the limit.

use crate::intake::session_snapshot::{SourceCatalog, normalized_supported_source};
use crate::reduce::AgentStatus;

/// The BLE service and characteristics the firmware advertises.
pub const SERVICE_UUID: &str = "0000beef-0000-1000-8000-00805f9b34fb";
pub const WRITE_CHARACTERISTIC_UUID: &str = "0000beef-0001-1000-8000-00805f9b34fb";
pub const NOTIFY_CHARACTERISTIC_UUID: &str = "0000beef-0002-1000-8000-00805f9b34fb";
/// The name the device advertises while it is looking for a host.
pub const ADVERTISED_DEVICE_NAME: &str = "Buddy";

pub const MAX_TOOL_NAME_BYTES: usize = 17;
pub const MAX_FRAME_BYTES: usize = 3 + MAX_TOOL_NAME_BYTES;
pub const MAX_WORKSPACE_NAME_BYTES: usize = 18;
pub const MAX_WORKSPACE_FRAME_BYTES: usize = 2 + MAX_WORKSPACE_NAME_BYTES;
pub const MAX_MESSAGE_PREVIEW_BYTES: usize = 16;
pub const MAX_MESSAGE_PREVIEW_FRAME_BYTES: usize = 4 + MAX_MESSAGE_PREVIEW_BYTES;
pub const MAX_MESSAGE_PREVIEW_SEGMENTS: u8 = 255;
pub const MAX_MODEL_NAME_BYTES: usize = 18;
pub const MAX_TOOL_HISTORY_NAME_BYTES: usize = 11;

pub const BRIGHTNESS_FRAME_MARKER: u8 = 0xFE;
pub const ORIENTATION_FRAME_MARKER: u8 = 0xFD;
pub const WORKSPACE_FRAME_MARKER: u8 = 0xFC;
pub const MESSAGE_PREVIEW_FRAME_MARKER: u8 = 0xFB;
pub const MODEL_FRAME_MARKER: u8 = 0xF9;
pub const STATS_FRAME_MARKER: u8 = 0xFA;
pub const SUBAGENT_FRAME_MARKER: u8 = 0xF8;
pub const EVENT_FRAME_MARKER: u8 = 0xF7;
pub const TIME_HINT_FRAME_MARKER: u8 = 0xF6;
pub const TOOL_HISTORY_FRAME_MARKER: u8 = 0xF5;

pub const APPROVE_CURRENT_PERMISSION_MARKER: u8 = 0xF0;
pub const DENY_CURRENT_PERMISSION_MARKER: u8 = 0xF1;
pub const SKIP_CURRENT_QUESTION_MARKER: u8 = 0xF2;

// Pairing: the reserved range, both directions.
pub const PAIR_REQUEST_MARKER: u8 = 0xE0;
pub const UNPAIR_MARKER: u8 = 0xE1;
pub const HOST_ID_LENGTH: usize = 6;
pub const PAIR_REQUEST_FRAME_BYTES: usize = 1 + HOST_ID_LENGTH;
pub const UNPAIR_FRAME_BYTES: usize = 1 + HOST_ID_LENGTH;
pub const PAIR_ACCEPTED_MARKER: u8 = 0xE0;
pub const PAIR_REJECTED_MARKER: u8 = 0xE1;
pub const PAIR_PENDING_MARKER: u8 = 0xE2;
pub const PAIR_RESPONSE_TIMEOUT_SECONDS: f64 = 2.5;
pub const PAIR_CONFIRM_TIMEOUT_SECONDS: u64 = 30;

pub const MIN_BRIGHTNESS_PERCENT: u8 = 10;
pub const MAX_BRIGHTNESS_PERCENT: u8 = 100;
pub const DEFAULT_BRIGHTNESS_PERCENT: u8 = 70;
/// The firmware's own inactivity timeout. A host must stay well under it.
pub const FIRMWARE_INACTIVITY_TIMEOUT_MS: u64 = 60_000;

/// A brightness asked for, clamped to what the firmware accepts. A value that is not a
/// number is the default, not zero: `0` would turn the screen off.
pub fn clamped_brightness_percent(percent: f64) -> u8 {
    if !percent.is_finite() {
        return DEFAULT_BRIGHTNESS_PERCENT;
    }
    let rounded = percent.round();
    let clamped = rounded.clamp(
        f64::from(MIN_BRIGHTNESS_PERCENT),
        f64::from(MAX_BRIGHTNESS_PERCENT),
    );
    clamped as u8
}

/// A mascot slot on the screen. The index is the on-wire `sourceId`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MascotId {
    Claude,
    Codex,
    Gemini,
    Cursor,
    Copilot,
    Trae,
    Qoder,
    Droid,
    Codebuddy,
    Stepfun,
    Opencode,
    Qwen,
    Antigravity,
    Workbuddy,
    Hermes,
    Kimi,
}

impl MascotId {
    pub fn raw_value(self) -> u8 {
        match self {
            Self::Claude => 0,
            Self::Codex => 1,
            Self::Gemini => 2,
            Self::Cursor => 3,
            Self::Copilot => 4,
            Self::Trae => 5,
            Self::Qoder => 6,
            Self::Droid => 7,
            Self::Codebuddy => 8,
            Self::Stepfun => 9,
            Self::Opencode => 10,
            Self::Qwen => 11,
            Self::Antigravity => 12,
            Self::Workbuddy => 13,
            Self::Hermes => 14,
            Self::Kimi => 15,
        }
    }

    /// The slot a raw uplink byte names, if it names one at all.
    pub fn from_raw(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::Claude,
            1 => Self::Codex,
            2 => Self::Gemini,
            3 => Self::Cursor,
            4 => Self::Copilot,
            5 => Self::Trae,
            6 => Self::Qoder,
            7 => Self::Droid,
            8 => Self::Codebuddy,
            9 => Self::Stepfun,
            10 => Self::Opencode,
            11 => Self::Qwen,
            12 => Self::Antigravity,
            13 => Self::Workbuddy,
            14 => Self::Hermes,
            15 => Self::Kimi,
            _ => return None,
        })
    }

    /// The canonical source name of this slot.
    pub fn source_name(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
            Self::Cursor => "cursor",
            Self::Copilot => "copilot",
            Self::Trae => "trae",
            Self::Qoder => "qoder",
            Self::Droid => "droid",
            Self::Codebuddy => "codebuddy",
            Self::Stepfun => "stepfun",
            Self::Opencode => "opencode",
            Self::Qwen => "qwen",
            Self::Antigravity => "antigravity",
            Self::Workbuddy => "workbuddy",
            Self::Hermes => "hermes",
            Self::Kimi => "kimi",
        }
    }

    /// Fold a source string — including the aliases — into one of the sixteen slots.
    ///
    /// The firmware has all sixteen assigned, so aliases share a slot rather than
    /// getting one of their own: Google Antigravity is Gemini-based and takes Gemini's;
    /// MiMo Code is an OpenCode fork and takes OpenCode's, exactly as `cursor-cli`
    /// folds onto `.cursor`.
    pub fn from_source(source: Option<&str>) -> Option<Self> {
        let canonical = normalized_supported_source(source, &SourceCatalog::default())?;
        Some(match canonical.as_str() {
            "claude" => Self::Claude,
            "codex" => Self::Codex,
            "gemini" | "google-antigravity" => Self::Gemini,
            "cursor" | "cursor-cli" => Self::Cursor,
            "copilot" => Self::Copilot,
            "trae" | "traecn" | "traecli" => Self::Trae,
            "qoder" | "qoder-cli" | "qoderwork" => Self::Qoder,
            "droid" => Self::Droid,
            "codebuddy" | "codybuddycn" => Self::Codebuddy,
            "stepfun" => Self::Stepfun,
            "opencode" | "mimo" => Self::Opencode,
            "qwen" => Self::Qwen,
            "antigravity" => Self::Antigravity,
            "workbuddy" => Self::Workbuddy,
            "hermes" => Self::Hermes,
            "kimi" => Self::Kimi,
            _ => return None,
        })
    }
}

/// The on-wire status code, matching the firmware's `statusToScene` table:
/// 0 → SLEEP, 1/2 → WORK (the tool name is drawn), 3 → ALERT, 4 → QUESTION.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MascotStatusCode {
    Idle,
    Processing,
    Running,
    WaitingApproval,
    WaitingQuestion,
}

impl MascotStatusCode {
    pub fn raw_value(self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Processing => 1,
            Self::Running => 2,
            Self::WaitingApproval => 3,
            Self::WaitingQuestion => 4,
        }
    }

    pub fn from_status(status: AgentStatus) -> Self {
        match status {
            AgentStatus::Idle => Self::Idle,
            AgentStatus::Processing => Self::Processing,
            AgentStatus::Running => Self::Running,
            AgentStatus::WaitingApproval => Self::WaitingApproval,
            AgentStatus::WaitingQuestion => Self::WaitingQuestion,
        }
    }
}

/// A control the screen sends back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlCommand {
    ApproveCurrentPermission,
    DenyCurrentPermission,
    SkipCurrentQuestion,
}

impl ControlCommand {
    pub fn raw_value(self) -> u8 {
        match self {
            Self::ApproveCurrentPermission => APPROVE_CURRENT_PERMISSION_MARKER,
            Self::DenyCurrentPermission => DENY_CURRENT_PERMISSION_MARKER,
            Self::SkipCurrentQuestion => SKIP_CURRENT_QUESTION_MARKER,
        }
    }

    pub fn from_raw(value: u8) -> Option<Self> {
        Some(match value {
            APPROVE_CURRENT_PERMISSION_MARKER => Self::ApproveCurrentPermission,
            DENY_CURRENT_PERMISSION_MARKER => Self::DenyCurrentPermission,
            SKIP_CURRENT_QUESTION_MARKER => Self::SkipCurrentQuestion,
            _ => return None,
        })
    }
}

/// The answer to a pairing request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairResponse {
    Accepted,
    Rejected,
    Pending,
}

impl PairResponse {
    pub fn raw_value(self) -> u8 {
        match self {
            Self::Accepted => PAIR_ACCEPTED_MARKER,
            Self::Rejected => PAIR_REJECTED_MARKER,
            Self::Pending => PAIR_PENDING_MARKER,
        }
    }

    pub fn from_raw(value: u8) -> Option<Self> {
        Some(match value {
            PAIR_ACCEPTED_MARKER => Self::Accepted,
            PAIR_REJECTED_MARKER => Self::Rejected,
            PAIR_PENDING_MARKER => Self::Pending,
            _ => return None,
        })
    }
}

/// Something the screen told the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UplinkEvent {
    /// The user tapped a mascot: show that session.
    Focus(MascotId),
    Command(ControlCommand),
    PairResponse(PairResponse),
}

impl UplinkEvent {
    /// Read one uplink payload.
    ///
    /// Pair responses are checked **first**, and that is safe only because a mascot id
    /// stays below `0xDF`. The ordering is in one place so growing the mascot range
    /// fails here rather than on a device.
    pub fn parse(payload: &[u8]) -> Option<Self> {
        let first = *payload.first()?;
        if let Some(response) = PairResponse::from_raw(first) {
            return Some(Self::PairResponse(response));
        }
        if let Some(mascot) = MascotId::from_raw(first) {
            return Some(Self::Focus(mascot));
        }
        ControlCommand::from_raw(first).map(Self::Command)
    }
}

/// Which way up the screen is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenOrientation {
    Up,
    Down,
}

impl ScreenOrientation {
    pub fn wire_value(self) -> u8 {
        match self {
            Self::Up => 0,
            Self::Down => 1,
        }
    }

    pub fn from_wire(value: u8) -> Self {
        if value == 1 { Self::Down } else { Self::Up }
    }

    /// Anything but `down` is `up`, so a settings file from another version still opens.
    pub fn from_settings(value: Option<&str>) -> Self {
        if value == Some("down") {
            Self::Down
        } else {
            Self::Up
        }
    }
}

/// `bytes` truncated to `limit` bytes, with no attempt to keep a character whole.
fn utf8_head(text: Option<&str>, limit: usize) -> Vec<u8> {
    let mut bytes: Vec<u8> = text.map(str::to_owned).unwrap_or_default().into_bytes();
    bytes.truncate(limit);
    bytes
}

/// The one frame that carries an agent's state (marker 0, i.e. no marker at all: the
/// mascot's own id is the first byte).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MascotFrame {
    pub mascot: MascotId,
    pub status: MascotStatusCode,
    pub tool_name: Option<String>,
}

impl MascotFrame {
    pub fn encode(&self) -> Vec<u8> {
        let tool = utf8_head(
            self.tool_name.as_deref().filter(|name| !name.is_empty()),
            MAX_TOOL_NAME_BYTES,
        );
        let mut frame = Vec::with_capacity(MAX_FRAME_BYTES);
        frame.push(self.mascot.raw_value());
        frame.push(self.status.raw_value());
        frame.push(tool.len() as u8);
        frame.extend_from_slice(&tool);
        frame
    }
}

/// The workspace name, for the card's header line.
pub fn encode_workspace(workspace: Option<&str>) -> Vec<u8> {
    let trimmed = workspace.map(str::trim).filter(|name| !name.is_empty());
    let bytes = utf8_head(trimmed, MAX_WORKSPACE_NAME_BYTES);
    let mut frame = Vec::with_capacity(MAX_WORKSPACE_FRAME_BYTES);
    frame.push(WORKSPACE_FRAME_MARKER);
    frame.push(bytes.len() as u8);
    frame.extend_from_slice(&bytes);
    frame
}

/// One segment of the last exchange. `is_user` rides in the top bit of the length byte,
/// so a segment can never be `0x80` or more bytes — the limit keeps it well under.
pub fn encode_message_preview(index: u8, total: u8, is_user: bool, text: Option<&str>) -> Vec<u8> {
    let normalized = text.filter(|text| !text.trim().is_empty());
    let bytes = utf8_head(normalized, MAX_MESSAGE_PREVIEW_BYTES);
    let mut frame = Vec::with_capacity(MAX_MESSAGE_PREVIEW_FRAME_BYTES);
    frame.push(MESSAGE_PREVIEW_FRAME_MARKER);
    frame.push(index);
    frame.push(total);
    let flags = (if is_user { 0x80 } else { 0x00 }) | bytes.len() as u8;
    frame.push(flags);
    frame.extend_from_slice(&bytes);
    frame
}

pub fn encode_brightness(percent: u8) -> Vec<u8> {
    vec![
        BRIGHTNESS_FRAME_MARKER,
        clamped_brightness_percent(f64::from(percent)),
    ]
}

pub fn encode_orientation(orientation: ScreenOrientation) -> Vec<u8> {
    vec![ORIENTATION_FRAME_MARKER, orientation.wire_value()]
}

pub fn encode_model(model_name: Option<&str>) -> Vec<u8> {
    let bytes = utf8_head(model_name, MAX_MODEL_NAME_BYTES);
    let mut frame = Vec::with_capacity(2 + MAX_MODEL_NAME_BYTES);
    frame.push(MODEL_FRAME_MARKER);
    frame.push(bytes.len() as u8);
    frame.extend_from_slice(&bytes);
    frame
}

pub fn encode_stats(
    active_sessions: usize,
    total_sessions: usize,
    tool_calls: usize,
    duration_minutes: usize,
) -> Vec<u8> {
    let tool_calls = tool_calls.min(u16::MAX as usize) as u16;
    vec![
        STATS_FRAME_MARKER,
        active_sessions.min(255) as u8,
        total_sessions.min(255) as u8,
        (tool_calls >> 8) as u8,
        (tool_calls & 0xFF) as u8,
        duration_minutes.min(255) as u8,
    ]
}

pub fn encode_subagent(count: usize) -> Vec<u8> {
    vec![SUBAGENT_FRAME_MARKER, count.min(15) as u8]
}

/// The transient animations the screen can play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenEvent {
    Start,
    Complete,
    Error,
    Approval,
    Submit,
}

impl ScreenEvent {
    pub fn event_id(self) -> u8 {
        match self {
            Self::Start => 0,
            Self::Complete => 1,
            Self::Error => 2,
            Self::Approval => 3,
            Self::Submit => 4,
        }
    }
}

pub fn encode_event(event: ScreenEvent) -> Vec<u8> {
    vec![EVENT_FRAME_MARKER, event.event_id()]
}

pub fn encode_time_hint(hour: usize) -> Vec<u8> {
    vec![TIME_HINT_FRAME_MARKER, hour.min(23) as u8]
}

pub fn encode_tool_history(index: usize, success: bool, tool_name: &str) -> Vec<u8> {
    let bytes = utf8_head(Some(tool_name), MAX_TOOL_HISTORY_NAME_BYTES);
    let mut frame = Vec::with_capacity(3 + MAX_TOOL_HISTORY_NAME_BYTES);
    frame.push(TOOL_HISTORY_FRAME_MARKER);
    frame.push(index.min(255) as u8);
    frame.push((if success { 0x80 } else { 0x00 }) | bytes.len() as u8);
    frame.extend_from_slice(&bytes);
    frame
}

pub fn encode_tool_history_clear() -> Vec<u8> {
    vec![TOOL_HISTORY_FRAME_MARKER, 0, 0]
}

fn host_id_frame(marker: u8, host_id: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(1 + HOST_ID_LENGTH);
    frame.push(marker);
    let mut padded = host_id.to_vec();
    padded.truncate(HOST_ID_LENGTH);
    padded.resize(HOST_ID_LENGTH, 0);
    frame.extend_from_slice(&padded);
    frame
}

pub fn encode_pair_request(host_id: &[u8]) -> Vec<u8> {
    host_id_frame(PAIR_REQUEST_MARKER, host_id)
}

pub fn encode_unpair(host_id: &[u8]) -> Vec<u8> {
    host_id_frame(UNPAIR_MARKER, host_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mascot_frame_is_id_status_length_and_bytes() {
        let frame = MascotFrame {
            mascot: MascotId::Codex,
            status: MascotStatusCode::Running,
            tool_name: Some("Bash".to_owned()),
        }
        .encode();
        assert_eq!(frame, vec![1, 2, 4, b'B', b'a', b's', b'h']);
        assert!(frame.len() <= MAX_FRAME_BYTES);
    }

    #[test]
    fn a_tool_name_is_cut_to_the_frame_limit_and_no_tool_is_an_empty_field() {
        let long = MascotFrame {
            mascot: MascotId::Claude,
            status: MascotStatusCode::Processing,
            tool_name: Some("a".repeat(40)),
        }
        .encode();
        assert_eq!(long.len(), MAX_FRAME_BYTES);
        assert_eq!(long[2], MAX_TOOL_NAME_BYTES as u8);

        let none = MascotFrame {
            mascot: MascotId::Claude,
            status: MascotStatusCode::Idle,
            tool_name: None,
        }
        .encode();
        assert_eq!(none, vec![0, 0, 0]);

        let empty = MascotFrame {
            mascot: MascotId::Claude,
            status: MascotStatusCode::Idle,
            tool_name: Some(String::new()),
        }
        .encode();
        assert_eq!(empty, none);
    }

    #[test]
    fn a_status_keeps_the_firmware_scene_numbers() {
        assert_eq!(
            MascotStatusCode::from_status(AgentStatus::Idle).raw_value(),
            0
        );
        assert_eq!(
            MascotStatusCode::from_status(AgentStatus::WaitingApproval).raw_value(),
            3
        );
        assert_eq!(
            MascotStatusCode::from_status(AgentStatus::WaitingQuestion).raw_value(),
            4
        );
        assert_eq!(
            MascotStatusCode::from_status(AgentStatus::Running).raw_value(),
            2
        );
        assert_eq!(
            MascotStatusCode::from_status(AgentStatus::Processing).raw_value(),
            1
        );
    }

    #[test]
    fn a_screen_name_folds_onto_the_sixteen_slots() {
        assert_eq!(
            MascotId::from_source(Some("claude")),
            Some(MascotId::Claude)
        );
        assert_eq!(
            MascotId::from_source(Some("google-antigravity")),
            Some(MascotId::Gemini),
            "Antigravity is Gemini-based and Gemini's slot is the only one left"
        );
        assert_eq!(
            MascotId::from_source(Some("cursor-cli")),
            Some(MascotId::Cursor)
        );
        assert_eq!(
            MascotId::from_source(Some("mimo")),
            Some(MascotId::Opencode)
        );
        assert_eq!(MascotId::from_source(Some("traecn")), Some(MascotId::Trae));
        assert_eq!(
            MascotId::from_source(Some("codybuddycn")),
            Some(MascotId::Codebuddy)
        );
        assert_eq!(MascotId::from_source(Some("nobody")), None);
        assert_eq!(MascotId::from_source(None), None);
    }

    #[test]
    fn every_slot_survives_a_round_trip_through_its_name_and_its_byte() {
        for raw in 0..=15u8 {
            let mascot = MascotId::from_raw(raw).expect("a slot");
            assert_eq!(mascot.raw_value(), raw);
            assert_eq!(
                MascotId::from_source(Some(mascot.source_name())),
                Some(mascot)
            );
        }
        assert_eq!(MascotId::from_raw(16), None);
        assert_eq!(
            MascotId::from_raw(0xE0),
            None,
            "the pairing range is not a mascot"
        );
    }

    #[test]
    fn the_workspace_frame_carries_its_name_and_nothing_when_blank() {
        assert_eq!(
            encode_workspace(Some("morrow")),
            vec![0xFC, 6, b'm', b'o', b'r', b'r', b'o', b'w']
        );
        assert_eq!(encode_workspace(Some("   ")), vec![0xFC, 0]);
        assert_eq!(encode_workspace(None), vec![0xFC, 0]);
        assert_eq!(
            encode_workspace(Some(&"x".repeat(40))).len(),
            MAX_WORKSPACE_FRAME_BYTES
        );
    }

    #[test]
    fn a_message_preview_puts_the_user_flag_in_the_top_bit() {
        let user = encode_message_preview(0, 2, true, Some("hi"));
        assert_eq!(user, vec![0xFB, 0, 2, 0x80 | 2, b'h', b'i']);
        let agent = encode_message_preview(1, 2, false, Some("hi"));
        assert_eq!(agent[3], 2);
        assert_eq!(
            encode_message_preview(0, 1, false, None),
            vec![0xFB, 0, 1, 0]
        );
        assert_eq!(
            encode_message_preview(0, 1, false, Some("  ")),
            vec![0xFB, 0, 1, 0],
            "whitespace is not a preview"
        );
    }

    #[test]
    fn brightness_is_clamped_and_two_bytes_long() {
        assert_eq!(encode_brightness(70), vec![0xFE, 70]);
        assert_eq!(encode_brightness(0), vec![0xFE, MIN_BRIGHTNESS_PERCENT]);
        assert_eq!(encode_brightness(255), vec![0xFE, MAX_BRIGHTNESS_PERCENT]);
        assert_eq!(clamped_brightness_percent(69.6), 70);
        assert_eq!(
            clamped_brightness_percent(f64::NAN),
            DEFAULT_BRIGHTNESS_PERCENT
        );
        assert_eq!(
            clamped_brightness_percent(f64::INFINITY),
            DEFAULT_BRIGHTNESS_PERCENT
        );
    }

    #[test]
    fn orientation_is_up_by_default() {
        assert_eq!(encode_orientation(ScreenOrientation::Down), vec![0xFD, 1]);
        assert_eq!(encode_orientation(ScreenOrientation::Up), vec![0xFD, 0]);
        assert_eq!(ScreenOrientation::from_wire(1), ScreenOrientation::Down);
        assert_eq!(ScreenOrientation::from_wire(9), ScreenOrientation::Up);
        assert_eq!(
            ScreenOrientation::from_settings(Some("down")),
            ScreenOrientation::Down
        );
        assert_eq!(
            ScreenOrientation::from_settings(Some("sideways")),
            ScreenOrientation::Up
        );
        assert_eq!(
            ScreenOrientation::from_settings(None),
            ScreenOrientation::Up
        );
    }

    #[test]
    fn model_and_stats_frames_carry_their_fields_in_order() {
        assert_eq!(
            encode_model(Some("opus")),
            vec![0xF9, 4, b'o', b'p', b'u', b's']
        );
        assert_eq!(encode_model(None), vec![0xF9, 0]);
        // 300 tool calls = 0x012C, big byte first.
        assert_eq!(
            encode_stats(2, 5, 300, 42),
            vec![0xFA, 2, 5, 0x01, 0x2C, 42]
        );
        assert_eq!(
            encode_stats(999, 999, 99_999, 999),
            vec![0xFA, 255, 255, 0xFF, 0xFF, 255]
        );
    }

    #[test]
    fn subagent_count_stops_at_the_firmware_limit() {
        assert_eq!(encode_subagent(3), vec![0xF8, 3]);
        assert_eq!(encode_subagent(99), vec![0xF8, 15]);
    }

    #[test]
    fn the_five_events_keep_their_ids_and_the_clock_stops_at_23() {
        assert_eq!(encode_event(ScreenEvent::Start), vec![0xF7, 0]);
        assert_eq!(encode_event(ScreenEvent::Submit), vec![0xF7, 4]);
        assert_eq!(encode_time_hint(9), vec![0xF6, 9]);
        assert_eq!(encode_time_hint(99), vec![0xF6, 23]);
    }

    #[test]
    fn a_tool_history_entry_carries_success_in_the_top_bit() {
        assert_eq!(
            encode_tool_history(2, true, "Bash"),
            vec![0xF5, 2, 0x80 | 4, b'B', b'a', b's', b'h']
        );
        assert_eq!(encode_tool_history(0, false, "Bash")[2], 4);
        assert_eq!(encode_tool_history_clear(), vec![0xF5, 0, 0]);
    }

    #[test]
    fn a_host_id_is_padded_and_cut_to_six_bytes() {
        assert_eq!(
            encode_pair_request(&[1, 2, 3, 4, 5, 6]),
            vec![0xE0, 1, 2, 3, 4, 5, 6]
        );
        assert_eq!(encode_pair_request(&[1, 2]), vec![0xE0, 1, 2, 0, 0, 0, 0]);
        assert_eq!(
            encode_pair_request(&[1, 2, 3, 4, 5, 6, 7]),
            vec![0xE0, 1, 2, 3, 4, 5, 6]
        );
        assert_eq!(encode_unpair(&[9]), vec![0xE1, 9, 0, 0, 0, 0, 0]);
        assert_eq!(
            encode_pair_request(&[1, 2, 3, 4, 5, 6]).len(),
            PAIR_REQUEST_FRAME_BYTES
        );
    }

    #[test]
    fn an_uplink_byte_is_read_as_a_pair_response_first() {
        // 0xE0/0xE1 are both a pair response and a pairing marker: the response wins.
        assert_eq!(
            UplinkEvent::parse(&[PAIR_ACCEPTED_MARKER]),
            Some(UplinkEvent::PairResponse(PairResponse::Accepted))
        );
        assert_eq!(
            UplinkEvent::parse(&[PAIR_PENDING_MARKER]),
            Some(UplinkEvent::PairResponse(PairResponse::Pending))
        );
        // 0xE2 is a response only.
        assert_eq!(
            UplinkEvent::parse(&[0xE2]),
            Some(UplinkEvent::PairResponse(PairResponse::Pending))
        );
    }

    #[test]
    fn a_mascot_byte_and_a_control_byte_are_read_in_their_own_ranges() {
        assert_eq!(
            UplinkEvent::parse(&[3]),
            Some(UplinkEvent::Focus(MascotId::Cursor))
        );
        assert_eq!(
            UplinkEvent::parse(&[DENY_CURRENT_PERMISSION_MARKER]),
            Some(UplinkEvent::Command(ControlCommand::DenyCurrentPermission))
        );
        assert_eq!(UplinkEvent::parse(&[0x55]), None);
        assert_eq!(UplinkEvent::parse(&[]), None);
    }
}
