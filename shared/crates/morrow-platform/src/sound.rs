//! Sound effects (#241): playing the cue `AttentionPolicy` decided on.
//!
//! The table below is `SoundManager.eventSounds` plus its launch chime, one row
//! per cue. A cue is a hook event name, and only some of them have a sound of
//! their own: `soundName(forCue:)` returns nil for the rest, and so does
//! [`cue_sound`] — the policy speaks in event names, not in sounds, so a cue
//! without a row is silent rather than an error.
//!
//! Like the other Unix adapters this asks a tool the desktop already ships
//! (`pw-play`, `paplay`, `canberra-gtk-play`) and spawns it detached: a sound
//! must never hold up the feed. Windows needs no tool — `PlaySoundW` is part of
//! the OS.
//!
//! Not ported here: the user's sound switches, quiet hours and the turn-failure
//! debounce. Those are `AttentionPolicy.settle` and belong with the rest of it
//! (#238); this module only plays what it is handed.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// One cue's sound. The WAV is compiled into the binary, so a packaged island
/// does not read the macOS tree at runtime.
pub struct Cue {
    /// The cue name `AttentionPolicy` speaks in — a hook event name.
    pub name: &'static str,
    /// The file name, kept for the temp copy and for the Settings preview.
    pub file: &'static str,
    pub wav: &'static [u8],
}

/// `SoundManager.eventSounds`, then its launch chime, in the same order.
pub const CUES: &[Cue] = &[
    Cue {
        name: "SessionStart",
        file: "8bit_start.wav",
        wav: include_bytes!("../../../../macos/Sources/Morrow/Resources/8bit_start.wav"),
    },
    // Both a round and a whole turn end on the same jingle.
    Cue {
        name: "TaskRoundComplete",
        file: "8bit_complete.wav",
        wav: include_bytes!("../../../../macos/Sources/Morrow/Resources/8bit_complete.wav"),
    },
    Cue {
        name: "Stop",
        file: "8bit_complete.wav",
        wav: include_bytes!("../../../../macos/Sources/Morrow/Resources/8bit_complete.wav"),
    },
    // A whole turn that died (API error, rate limit), not one failed tool call.
    Cue {
        name: "StopFailure",
        file: "8bit_error.wav",
        wav: include_bytes!("../../../../macos/Sources/Morrow/Resources/8bit_error.wav"),
    },
    Cue {
        name: "PermissionRequest",
        file: "8bit_approval.wav",
        wav: include_bytes!("../../../../macos/Sources/Morrow/Resources/8bit_approval.wav"),
    },
    Cue {
        name: "UserPromptSubmit",
        file: "8bit_submit.wav",
        wav: include_bytes!("../../../../macos/Sources/Morrow/Resources/8bit_submit.wav"),
    },
    // The app's own cue, not a hook event: no hook may ring it.
    Cue {
        name: "AppLaunch",
        file: "8bit_boot.wav",
        wav: include_bytes!("../../../../macos/Sources/Morrow/Resources/8bit_boot.wav"),
    },
];

/// The sound for a cue, or `None` when the cue has none of its own.
pub fn cue_sound(cue: &str) -> Option<&'static Cue> {
    CUES.iter().find(|sound| sound.name == cue)
}

/// Who plays the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Player {
    /// A desktop tool, spawned with the file as its argument.
    Tool(&'static str),
    /// The OS itself; there is no tool to find.
    #[cfg(windows)]
    System,
}

/// The Unix tools that can play a WAV, best first: PipeWire, then PulseAudio,
/// then the freedesktop event-sound player.
#[cfg(not(windows))]
const TOOLS: &[&str] = &["pw-play", "paplay", "canberra-gtk-play"];

/// Who plays a sound here, if anyone. `has` answers "is this program there",
/// so the Capability probe and the feed ask the same question of the same list.
#[cfg(windows)]
pub fn player(_has: impl Fn(&str) -> bool) -> Option<Player> {
    Some(Player::System)
}

#[cfg(not(windows))]
pub fn player(has: impl Fn(&str) -> bool) -> Option<Player> {
    TOOLS
        .iter()
        .copied()
        .find(|tool| has(tool))
        .map(Player::Tool)
}

/// Who plays a sound here, asking `PATH` rather than an injected probe: the
/// feed has no `Environment` of its own, and the answer is the same one the
/// Capability table gives.
pub fn player_on_path() -> Option<Player> {
    let path = std::env::var_os("PATH")?;
    let dirs: Vec<PathBuf> = std::env::split_paths(&path).collect();
    player(|tool| dirs.iter().any(|dir| dir.join(tool).is_file()))
}

/// Play a cue. A cue with no sound of its own is silent, and so is a machine
/// with no player: neither is an error the caller has to handle.
pub fn play(player: Player, cue: &str) -> Result<(), String> {
    let Some(sound) = cue_sound(cue) else {
        return Ok(());
    };
    let path = materialise(sound)?;
    match player {
        Player::Tool(program) => spawn(program, &path),
        #[cfg(windows)]
        Player::System => crate::windows::sound::play(&path),
    }
}

/// Where the copies live: the runtime dir when the session has one (per user,
/// 0700), otherwise the temp dir. A shared `/tmp` would let the first user's
/// copy lock out the next one.
fn sound_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map_or_else(std::env::temp_dir, PathBuf::from)
        .join("morrow-sounds")
}

/// The bundled WAV as a file the players can open. Written into the temp dir
/// and rewritten only when the bytes differ, so an upgrade replaces the sound
/// instead of playing the old one.
fn materialise(sound: &Cue) -> Result<PathBuf, String> {
    let dir = sound_dir();
    std::fs::create_dir_all(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    let path = dir.join(sound.file);
    if std::fs::read(&path).ok().as_deref() != Some(sound.wav) {
        std::fs::write(&path, sound.wav).map_err(|error| format!("{}: {error}", path.display()))?;
    }
    Ok(path)
}

/// Start the player and return at once. The child is reaped on its own thread:
/// waiting here would hold the feed, and not waiting at all would leave a
/// zombie behind every turn.
fn spawn(program: &str, path: &Path) -> Result<(), String> {
    let mut command = Command::new(program);
    // `canberra` takes the file behind a flag; the PipeWire and PulseAudio
    // players take it as the only argument.
    if program == "canberra-gtk-play" {
        command.arg("-f");
    }
    let mut child = command
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("{program}: {error}"))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use morrow_core::attention::{TURN_FAILED, sound_cue};

    /// Every cue the policy can hand the shell, from the policy's own names.
    const SPOKEN: &[&str] = &[
        sound_cue::REQUEST,
        sound_cue::SESSION_START,
        sound_cue::PROMPT,
        sound_cue::TURN_END,
        sound_cue::LAUNCH,
        TURN_FAILED,
    ];

    #[test]
    fn every_cue_the_policy_speaks_has_a_bundled_sound() {
        for cue in SPOKEN {
            let sound = cue_sound(cue).unwrap_or_else(|| panic!("{cue} has no sound"));
            assert!(!sound.wav.is_empty(), "{} is empty", sound.file);
        }
    }

    /// The policy hands over event names, so most of them have no sound of
    /// their own. macOS answers nil for those and plays nothing.
    #[test]
    fn a_cue_with_no_sound_of_its_own_is_silent() {
        assert!(cue_sound("PostToolUse").is_none());
        assert!(cue_sound("Notification").is_none());
        assert!(cue_sound("").is_none());
    }

    /// A WAV that is not a WAV is a silent failure on every player, so the
    /// bytes are checked where they are compiled in.
    #[test]
    fn every_bundled_file_is_a_wav() {
        for sound in CUES {
            assert_eq!(
                &sound.wav[..4],
                b"RIFF",
                "{} is not a RIFF file",
                sound.file
            );
            assert_eq!(&sound.wav[8..12], b"WAVE", "{} is not a WAV", sound.file);
        }
    }

    #[test]
    fn the_first_player_on_the_machine_wins() {
        assert_eq!(
            player(|tool| tool == "paplay"),
            Some(Player::Tool("paplay"))
        );
        // PipeWire beats PulseAudio when both are there.
        assert_eq!(
            player(|tool| tool == "paplay" || tool == "pw-play"),
            Some(Player::Tool("pw-play"))
        );
        assert_eq!(player(|_| false), None, "no player, no sound");
    }

    /// The bytes reach a real file the players can open, and a second call
    /// does not rewrite it.
    #[test]
    fn the_sound_lands_in_a_file_the_player_can_open() {
        let sound = cue_sound(sound_cue::REQUEST).expect("approval sound");
        let path = materialise(sound).expect("materialise");
        assert_eq!(std::fs::read(&path).expect("read back"), sound.wav);
        let first = std::fs::metadata(&path).expect("metadata").modified().ok();
        materialise(sound).expect("materialise again");
        assert_eq!(
            std::fs::metadata(&path).expect("metadata").modified().ok(),
            first,
            "an unchanged sound is not rewritten"
        );
    }

    /// An unknown cue never reaches a player, so nothing is spawned.
    #[test]
    fn an_unknown_cue_plays_nothing() {
        assert_eq!(play(Player::Tool("false"), "PostToolUse"), Ok(()));
    }
}
