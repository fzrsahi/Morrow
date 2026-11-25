//! The remote install must be the local `ConfigInstaller` rules, not a second
//! implementation: JSON5 comments, YAML managed regions and Codex's TOML flag.

use std::collections::BTreeMap;

use base64::Engine;
use morrow_core::remote::host::SshHost;
use morrow_core::remote::install::{
    self, RemoteConfigPlan, RemoteFileWrite, RemoteInstallInput,
};
use morrow_core::installer::{claude_hooks, codex_config, opencode, targets, yaml_hooks};

const BRIDGE: &str = "python3 ~/.codeisland/codeisland-remote-hook.py";
const PLUGIN_REF: &str = "~/.codeisland/codeisland-opencode-remote.js";
const SOCKET: &str = "/tmp/codeisland-1000/island.sock";

fn inputs(existing: BTreeMap<String, String>) -> (SshHost, BTreeMap<String, String>) {
    let host = SshHost {
        id: "h-01".into(),
        name: "devbox".into(),
        host: "10.0.0.1".into(),
        agents_run_as_other_users: true,
        ..SshHost::default()
    };
    (host, existing)
}

fn contents<'a>(plan: &'a RemoteConfigPlan, path: &str) -> Option<&'a str> {
    plan.writes
        .iter()
        .find(|write| write.path == path)
        .map(|write| write.contents.as_str())
}

fn fixture() -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            ".claude/settings.json".into(),
            "// my settings\n{\"hooks\":{\"Stop\":[{\"hooks\":[{\"type\":\"command\",\"command\":\"mine\"}]}]}}\n".into(),
        ),
        (
            ".qoder/settings.json".into(),
            "// qoder keeps this\n{\"model\":\"x\"}\n".into(),
        ),
        (".codex/config.toml".into(), "# my config\n[features]\nhooks = false\n".into()),
        (".hermes/config.yaml".into(), "# user's yaml\ntheme: dark\n".into()),
        (".config/opencode/opencode.jsonc".into(), "// opencode\n{\"model\":\"x\"}\n".into()),
    ])
}

#[test]
fn every_config_file_comes_from_the_local_installer() {
    let existing = fixture();
    let (host, existing) = inputs(existing);
    let hook_source = "print('remote hook')";
    let plugin_source = "var p = \"__MORROW_SOCKET_PATH__\"; var owner = \"__MORROW_SOCKET_OWNER_UID__\"; var id = \"__MORROW_REMOTE_HOST_ID__\"; var name = \"__MORROW_REMOTE_HOST_NAME__\";";
    let plan = install::plan(&RemoteInstallInput {
        host: &host,
        remote_socket_path: SOCKET,
        hook_source,
        plugin_source,
        existing: &existing,
    });

    assert_eq!(
        contents(&plan, ".claude/settings.json"),
        claude_hooks::install(
            existing.get(".claude/settings.json").map(String::as_str),
            BRIDGE,
            Some("0.4.4"),
        )
        .map(|outcome| outcome.text)
        .as_deref()
    );
    assert!(contents(&plan, ".claude/settings.json").unwrap().contains("// my settings"));

    for source in ["qoder", "codebuddy", "codex"] {
        let target = targets::target_for(source).unwrap();
        assert_eq!(
            contents(&plan, target.path),
            targets::install(target, existing.get(target.path).map(String::as_str), BRIDGE)
                .map(|outcome| outcome.text)
                .as_deref(),
            "{source}"
        );
    }
    assert_eq!(
        contents(&plan, ".codex/config.toml"),
        codex_config::enabling_hooks(existing.get(".codex/config.toml").unwrap()).as_deref()
    );
    for source in yaml_hooks::SOURCES {
        assert_eq!(
            contents(&plan, source.path),
            yaml_hooks::install(
                source,
                existing.get(source.path).map(String::as_str).unwrap_or(""),
                BRIDGE,
            )
            .ok()
            .as_deref(),
            "{}",
            source.name
        );
    }
    assert_eq!(
        contents(&plan, ".config/opencode/opencode.jsonc"),
        opencode::merge_plugin_ref(
            existing.get(".config/opencode/opencode.jsonc").map(String::as_str),
            PLUGIN_REF,
        )
        .as_deref()
    );
    assert!(contents(&plan, ".config/opencode/opencode.jsonc").unwrap().contains("// opencode"));
}

#[test]
fn plugin_placeholders_are_filled_and_a_disagreeing_plugin_is_refused() {
    let existing = fixture();
    let (host, existing) = inputs(existing);
    let source = "var p = \"__MORROW_SOCKET_PATH__\"; var owner = \"__MORROW_SOCKET_OWNER_UID__\"; var id = \"__MORROW_REMOTE_HOST_ID__\"; var name = \"__MORROW_REMOTE_HOST_NAME__\";";
    let input = RemoteInstallInput {
        host: &host,
        remote_socket_path: SOCKET,
        hook_source: "print('hook')",
        plugin_source: source,
        existing: &existing,
    };
    let plan = install::plan(&input);
    let plugin = contents(&plan, ".codeisland/codeisland-opencode-remote.js").unwrap();
    assert!(plugin.contains(SOCKET));
    assert!(plugin.contains("1000"));
    assert!(plugin.contains("h-01"));
    assert!(plugin.contains("devbox"));
    assert!(!plugin.contains("__MORROW_"));

    let input = RemoteInstallInput {
        plugin_source: "var p = \"__MISSING__\";",
        ..input
    };
    let plan = install::plan(&input);
    assert!(plan.skipped.iter().any(|reason| reason.contains("placeholder mismatch")));
    assert!(contents(&plan, ".codeisland/codeisland-opencode-remote.js").is_none());
}

#[test]
fn a_yaml_file_this_installer_refuses_is_skipped_not_guessed() {
    let mut existing = fixture();
    existing.insert(".trae/traecli.yaml".into(), "outer:\n  hooks:\n    x: mine\n".into());
    let (host, existing) = inputs(existing);
    let plan = install::plan(&RemoteInstallInput {
        host: &host,
        remote_socket_path: SOCKET,
        hook_source: "print('hook')",
        plugin_source: "var p = \"__MORROW_SOCKET_PATH__\"; var owner = \"__MORROW_SOCKET_OWNER_UID__\"; var id = \"__MORROW_REMOTE_HOST_ID__\"; var name = \"__MORROW_REMOTE_HOST_NAME__\";",
        existing: &existing,
    });
    assert!(plan.skipped.iter().any(|name| name == "Trae CLI"));
    assert!(contents(&plan, ".trae/traecli.yaml").is_none());
}

#[test]
fn install_script_carries_the_exact_planned_bytes() {
    let existing = fixture();
    let (host, existing) = inputs(existing);
    let plan = install::plan(&RemoteInstallInput {
        host: &host,
        remote_socket_path: SOCKET,
        hook_source: "print('remote hook')",
        plugin_source: "var p = \"__MORROW_SOCKET_PATH__\"; var owner = \"__MORROW_SOCKET_OWNER_UID__\"; var id = \"__MORROW_REMOTE_HOST_ID__\"; var name = \"__MORROW_REMOTE_HOST_NAME__\";",
        existing: &existing,
    });
    let script = install::install_script(&plan);
    for write in &plan.writes {
        let encoded = base64::engine::general_purpose::STANDARD.encode(write.contents.as_bytes());
        assert!(script.contains(&encoded), "{} is missing", write.path);
        assert!(script.contains(&format!("\"{}\"", write.path)));
    }
    assert!(plan.writes.contains(&RemoteFileWrite {
        path: ".codeisland/codeisland-remote-hook.py".into(),
        contents: "print('remote hook')".into(),
        mode: 0o755,
    }));
}
