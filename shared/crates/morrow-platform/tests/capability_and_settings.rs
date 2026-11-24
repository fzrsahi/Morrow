//! The Capability table, autostart and settings storage (#101): what Settings
//! promises to say out loud, and the two files it owns.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use morrow_platform::autostart;
use morrow_platform::capability::{ADAPTERS, Capability, Environment, Feature, Os, adapter, table};
use serde_json::json;

fn temp(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("a clock")
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("morrow-cap-{tag}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&root).expect("root");
    root
}

fn x11(config_home: PathBuf) -> Environment {
    Environment {
        os: morrow_platform::capability::Os::Linux,
        session_type: Some("x11".into()),
        has_session_bus: true,
        path_tools: vec!["/usr/bin".into()],
        config_home,
        has_portal_shortcuts: false,
    }
}

fn wayland(config_home: PathBuf) -> Environment {
    Environment {
        os: morrow_platform::capability::Os::Linux,
        session_type: Some("wayland".into()),
        ..x11(config_home)
    }
}

fn windows(config_home: PathBuf) -> Environment {
    Environment {
        os: Os::Windows,
        session_type: None,
        has_session_bus: false,
        path_tools: Vec::new(),
        config_home,
        has_portal_shortcuts: false,
    }
}

// MARK: - #235: a row may not claim more than the code does

/// Every row that is not `Unsupported` has to name the adapter behind it.
/// The registry is an explicit list, not a search, so a feature cannot become
/// "supported" by accident — and a feature that loses its adapter has to say so.
#[test]
fn no_row_claims_capability_without_a_registered_adapter() {
    let mut lies = Vec::new();
    for environment in [
        x11(temp("guard-x11")),
        wayland(temp("guard-wayland")),
        windows(temp("guard-windows")),
    ] {
        for feature in Feature::ALL {
            let capability = feature.probe(&environment);
            if matches!(capability, Capability::Unsupported(_)) {
                continue;
            }
            if adapter(environment.os, *feature).is_none() {
                lies.push(format!(
                    "{} on {:?} says {}",
                    feature.name(),
                    environment.os,
                    capability.label()
                ));
            }
        }
    }
    assert!(
        lies.is_empty(),
        "these rows claim a capability with no adapter registered (#235):\n  {}",
        lies.join("\n  ")
    );
}

/// The registry is only worth reading if the files it names are there.
#[test]
fn every_registered_adapter_names_a_file_that_exists() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut seen = std::collections::BTreeSet::new();
    for entry in ADAPTERS {
        assert!(
            root.join(entry.module).is_file(),
            "{} on {:?} names {}, which is not in the tree",
            entry.feature.name(),
            entry.os,
            entry.module
        );
        assert!(
            seen.insert((format!("{:?}", entry.os), entry.feature.name())),
            "{} on {:?} is registered twice",
            entry.feature.name(),
            entry.os
        );
    }
    assert!(!ADAPTERS.is_empty(), "the registry is the audit trail");
}

// MARK: - The table

#[test]
fn every_feature_gets_a_row_and_a_label() {
    let env = x11(temp("table"));
    let rows = table(&env);
    assert_eq!(rows.len(), Feature::ALL.len());
    assert!(rows.len() >= 21, "the plan's whole table is covered");
    for row in &rows {
        assert!(!row.feature.is_empty());
        assert!(!row.capability.label().is_empty());
        // Anything short of Supported has to explain itself.
        if !row.capability.is_supported() {
            assert!(
                row.capability
                    .reason()
                    .is_some_and(|reason| !reason.is_empty()),
                "{} says {} with no reason",
                row.feature,
                row.capability.label()
            );
        }
    }
}

#[test]
fn a_wayland_session_has_no_hotkey_to_grab_and_says_so() {
    // The hotkey is not simply gone: it needs a portal, or a desktop binding.
    // The adapters that do not exist at all are covered above, on both
    // sessions, because "Wayland lacks it" and "nobody wrote it" are
    // different answers and only one of them is Wayland's fault.
    let hotkey = Feature::GlobalHotkey.probe(&wayland(temp("wayland")));
    assert!(matches!(hotkey, Capability::Unsupported(_)));
    assert!(hotkey.reason().unwrap().contains("morrow toggle"));
}

#[test]
fn x11_supports_what_wayland_cannot() {
    let env = x11(temp("x11-features"));
    for feature in [Feature::IslandReposition, Feature::GlobalHotkey] {
        assert_eq!(
            feature.probe(&env),
            Capability::Supported,
            "{} on X11",
            feature.name()
        );
    }
}

/// #235: a feature with no adapter says so on every session, and says why. The
/// list is the audit's own: none of these has a line of code behind it yet, so
/// none of them may borrow X11's name to look supported.
#[test]
fn adapters_that_do_not_exist_yet_are_unsupported_everywhere() {
    for environment in [x11(temp("absent-x11")), wayland(temp("absent-wayland"))] {
        for feature in [
            Feature::TerminalVisibility,
            Feature::PointerMonitor,
            Feature::Fullscreen,
            Feature::SyntheticInput,
            Feature::SystemStats,
            Feature::Geolocation,
            Feature::Secrets,
            Feature::ClaudeCredentials,
            Feature::Ble,
            Feature::Updater,
        ] {
            let capability = feature.probe(&environment);
            assert!(
                matches!(capability, Capability::Unsupported(_)),
                "{} has no adapter, so it cannot say {}",
                feature.name(),
                capability.label()
            );
            assert!(
                capability.reason().is_some_and(|reason| !reason.is_empty()),
                "{} has to say why it is missing",
                feature.name()
            );
        }
    }
}

#[test]
fn a_missing_appindicator_is_reported_not_fatal() {
    // No library on PATH: the tray is unsupported, and everything else still
    // answers — the panel keeps working without a tray icon.
    let env = x11(temp("tray"));
    let tray = Feature::Tray.probe(&env);
    assert!(matches!(tray, Capability::Unsupported(_)), "{tray:?}");
    assert!(
        Feature::IslandReposition.probe(&env).is_supported(),
        "one missing adapter does not take the others down"
    );
}

#[test]
fn no_session_bus_marks_the_bus_bound_features() {
    let mut env = x11(temp("no-bus"));
    env.has_session_bus = false;
    for feature in [
        Feature::NowPlaying,
        Feature::SessionLock,
        Feature::Secrets,
        Feature::Ble,
    ] {
        assert!(
            matches!(feature.probe(&env), Capability::Unsupported(_)),
            "{} without a bus",
            feature.name()
        );
    }
    // Volume does not need the bus: it shells out.
    assert!(matches!(
        Feature::SystemVolume.probe(&env),
        Capability::Unsupported(_) | Capability::Supported
    ));
}

#[test]
fn sound_effects_are_supported_only_when_a_player_is_on_path() {
    // #241: the row follows the same `PATH` lookup the feed uses, so Settings
    // cannot promise a sound the shell has no way to play.
    let root = temp("sound");
    let bin = root.join("bin");
    fs::create_dir_all(&bin).expect("bin");

    let mut env = x11(temp("sound-env"));
    env.path_tools = vec![bin.to_string_lossy().into_owned()];
    assert!(
        matches!(Feature::Audio.probe(&env), Capability::Unsupported(_)),
        "no player, no sound"
    );

    // A file named like a player is what the probe looks for.
    fs::write(bin.join("pw-play"), "#!/bin/sh\n").expect("player");
    assert_eq!(Feature::Audio.probe(&env), Capability::Supported);
}

#[test]
fn missing_credentials_are_unsupported_rather_than_promised() {
    // #235: nothing reads the credentials file yet, so a file that happens to
    // exist is not a reason to say the feature works.
    let env = x11(temp("creds"));
    let capability = Feature::ClaudeCredentials.probe(&env);
    assert!(
        matches!(capability, Capability::Unsupported(_)),
        "{capability:?}"
    );
    // The file's presence changes nothing: there is no reader to consult it.
    fs::write(env.config_home.join(".credentials.json"), "{}").expect("write credentials");
    assert_eq!(Feature::ClaudeCredentials.probe(&env), capability);
}

#[test]
fn a_present_credentials_file_makes_the_row_supported() {
    // #306: the row is backed by `quota::read_credentials`. With the file it
    // reads in place, the reader exists and the row honestly says Supported.
    let root = temp("creds-supported");
    let path = root.join(".credentials.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        r#"{"claudeAiOauth":{"accessToken":"not-read-here"}}"#,
    )
    .unwrap();
    let mut env = x11(root);
    env.claude_credentials = path;
    assert_eq!(
        Feature::ClaudeCredentials.probe(&env),
        Capability::Supported
    );
}

#[test]
fn dragging_the_island_is_a_capability_not_a_silent_failure() {
    // X11 and Windows let the app place its own window; Wayland hands that to
    // the compositor, so the drag is stated as unavailable in Settings (#196).
    assert_eq!(
        Feature::IslandReposition.probe(&x11(temp("drag-x11"))),
        Capability::Supported
    );
    let wayland = Feature::IslandReposition.probe(&wayland(temp("drag-wayland")));
    assert!(matches!(wayland, Capability::Unsupported(_)), "{wayland:?}");
    assert!(
        wayland
            .reason()
            .is_some_and(|reason| reason.to_ascii_lowercase().contains("wayland")),
        "the row has to say why drag is unavailable: {wayland:?}"
    );
    // The row is actually rendered, not just probeable.
    let rows = table(&x11(temp("drag-table")));
    assert!(
        rows.iter()
            .any(|row| row.feature == Feature::IslandReposition.name())
    );
}

// MARK: - Autostart

#[test]
fn autostart_writes_one_desktop_entry_and_takes_it_back() {
    let root = temp("autostart");
    let config = root.join(".config");
    fs::create_dir_all(&config).expect("config");

    assert!(!autostart::status(&config).installed);
    assert!(autostart::install(&config, PathBuf::from("/usr/bin/morrow-app").as_path()).unwrap());
    let text = fs::read_to_string(autostart::path(&config)).expect("read");
    assert!(text.starts_with("[Desktop Entry]"));
    assert!(text.contains("Exec=/usr/bin/morrow-app"));
    assert!(text.contains("Type=Application"));
    assert!(autostart::status(&config).installed);

    // Second install: the same bytes, nothing written.
    assert!(!autostart::install(&config, PathBuf::from("/usr/bin/morrow-app").as_path()).unwrap());
    assert!(autostart::uninstall(&config).unwrap());
    assert!(!autostart::status(&config).installed);
    assert!(!autostart::uninstall(&config).unwrap(), "already gone");
}

#[test]
fn a_path_with_spaces_is_quoted_in_the_entry() {
    let root = temp("autostart-space");
    let config = root.join(".config");
    autostart::install(&config, PathBuf::from("/home/a b/morrow-app").as_path()).unwrap();
    let text = fs::read_to_string(autostart::path(&config)).expect("read");
    assert!(text.contains("Exec=\"/home/a b/morrow-app\""), "{text}");
}

// MARK: - Settings

#[test]
fn settings_round_trip_and_keep_every_other_key() {
    let root = temp("settings");
    let dir = root.join(".codeisland");
    fs::create_dir_all(&dir).expect("dir");
    fs::write(
        morrow_platform::settings::path(&dir),
        "{\n  \"theme\": \"dark\",\n  \"sound\": \"8bit\"\n}\n",
    )
    .expect("write");

    assert!(morrow_platform::settings::set(&dir, "sound", &json!("modern")).unwrap());
    let loaded = morrow_platform::settings::load(&dir).unwrap();
    assert_eq!(loaded["sound"], json!("modern"));
    assert_eq!(loaded["theme"], json!("dark"), "an unrelated key is kept");
    let text = fs::read_to_string(morrow_platform::settings::path(&dir)).expect("read");
    assert!(text.ends_with('\n'), "the file's own newline stays");

    // The same value again changes nothing.
    assert!(!morrow_platform::settings::set(&dir, "sound", &json!("modern")).unwrap());
}

#[test]
fn settings_of_a_missing_file_are_an_empty_object() {
    let root = temp("settings-empty");
    let dir = root.join(".codeisland");
    let loaded = morrow_platform::settings::load(&dir).unwrap();
    assert_eq!(loaded, json!({}));

    assert!(morrow_platform::settings::set(&dir, "sound", &json!("8bit")).unwrap());
    let text = fs::read_to_string(morrow_platform::settings::path(&dir)).expect("read");
    assert!(text.starts_with('{'), "{text}");
}

#[test]
fn settings_that_do_not_parse_are_never_rewritten() {
    let root = temp("settings-broken");
    let dir = root.join(".codeisland");
    fs::create_dir_all(&dir).expect("dir");
    let broken = "{ \"theme\": ";
    fs::write(morrow_platform::settings::path(&dir), broken).expect("write");

    let error = morrow_platform::settings::set(&dir, "sound", &json!("8bit")).expect_err("refuses");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert_eq!(
        fs::read_to_string(morrow_platform::settings::path(&dir)).expect("read"),
        broken
    );
}
