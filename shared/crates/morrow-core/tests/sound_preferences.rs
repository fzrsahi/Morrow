//! The Sound page's document becomes the policy's `SoundPreferences` (#462).
//! Defaults here keep the shell's pre-preference behaviour: every cue is on
//! until the user actually writes a switch.

use serde_json::{json, Value};

use morrow_core::attention::{
    audible_sound, sound_preferences, sound_setting, sound_volume_percent, SoundHold,
};

fn cue_audible(settings: &Value, cue: &str) -> bool {
    audible_sound(Some(cue), &sound_preferences(settings), &SoundHold::default()).is_some()
}

#[test]
fn no_preferences_keeps_every_cue_audible_at_full_volume() {
    let empty = json!({});
    for (cue, _) in sound_setting::CUES {
        assert!(cue_audible(&empty, cue), "{cue} is silent without a setting");
    }
    // A cue with no sound of its own never rings, however the switches are set.
    assert!(!cue_audible(&empty, "PostToolUse"));
    assert_eq!(sound_volume_percent(&empty), 100);
}

#[test]
fn the_master_switch_silences_every_cue() {
    let settings = json!({ "soundEnabled": false });
    for (cue, _) in sound_setting::CUES {
        assert!(!cue_audible(&settings, cue), "{cue} rang with sound off");
    }
}

#[test]
fn a_per_event_switch_only_silences_its_own_cue() {
    let settings = json!({
        "soundTaskComplete": false,
        "soundBoot": false,
    });
    assert!(!cue_audible(&settings, "Stop"));
    assert!(!cue_audible(&settings, "TaskRoundComplete"));
    assert!(!cue_audible(&settings, "AppLaunch"));
    assert!(cue_audible(&settings, "SessionStart"));
    assert!(cue_audible(&settings, "PermissionRequest"));
    assert!(cue_audible(&settings, "StopFailure"));
}

#[test]
fn volume_is_clamped_and_bad_values_keep_full_volume() {
    assert_eq!(sound_volume_percent(&json!({ "soundVolume": 0 })), 0);
    assert_eq!(sound_volume_percent(&json!({ "soundVolume": 25 })), 25);
    assert_eq!(sound_volume_percent(&json!({ "soundVolume": 140 })), 100);
    assert_eq!(sound_volume_percent(&json!({ "soundVolume": -5 })), 100);
    assert_eq!(sound_volume_percent(&json!({ "soundVolume": "loud" })), 100);
}
