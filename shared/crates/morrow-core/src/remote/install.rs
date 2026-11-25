//! Remote hook installation as a plan over text.
//!
//! Swift runs a second, 1200-line Python copy of `ConfigInstaller` on the
//! host. Here the remote command carries the result of the same core installer
//! functions the local app uses, so JSON5 comments, YAML managed regions and
//! Codex's TOML flag cannot drift between the two paths.

use std::collections::BTreeMap;

use base64::Engine;
use serde_json::Value;

use super::host::{SshHost, quoted_literal};
use crate::installer::claude_hooks;
use crate::installer::codex_config;
use crate::installer::opencode;
use crate::installer::targets;
use crate::installer::yaml_hooks;

pub const CONFIG_MARKER: &str = "CODEISLAND-CONFIG ";
const HOOK_PATH: &str = ".codeisland/codeisland-remote-hook.py";
const HOOK_COMMAND: &str = "python3 ~/.codeisland/codeisland-remote-hook.py";
const PLUGIN_PATH: &str = ".codeisland/codeisland-opencode-remote.js";

pub struct RemoteInstallInput<'a> {
    pub host: &'a SshHost,
    pub remote_socket_path: &'a str,
    pub hook_source: &'a str,
    pub plugin_source: &'a str,
    /// Config files relative to the remote home, with their current bytes.
    /// A missing key means the file does not exist there.
    pub existing: &'a BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteFileWrite {
    pub path: String,
    pub contents: String,
    pub mode: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteConfigPlan {
    pub writes: Vec<RemoteFileWrite>,
    pub skipped: Vec<String>,
    pub summary: String,
}

/// The files read before an install. Both OpenCode filenames are probed: the
/// one that exists decides which file gets the plugin reference.
pub fn config_paths() -> &'static [&'static str] {
    &[
        ".claude/settings.json",
        ".qoder/settings.json",
        ".codebuddy/settings.json",
        ".codex/hooks.json",
        ".codex/config.toml",
        ".hermes/config.yaml",
        ".trae/traecli.yaml",
        ".config/opencode/opencode.jsonc",
        ".config/opencode/opencode.json",
    ]
}

/// Parse the snapshot line printed by the host's config reader.
pub fn config_snapshot(output: &str) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    let Some(line) = output.lines().rev().find(|line| line.starts_with(CONFIG_MARKER)) else {
        return files;
    };
    let Ok(Value::Object(object)) = serde_json::from_str::<Value>(line[CONFIG_MARKER.len()..].trim()) else {
        return files;
    };
    for (path, value) in object {
        if let Some(text) = value.as_str() {
            files.insert(path, text.to_owned());
        }
    }
    files
}

fn get<'a>(existing: &'a BTreeMap<String, String>, path: &str) -> Option<&'a str> {
    existing.get(path).map(String::as_str)
}

/// Fill the plugin's placeholders, refusing a source that disagrees with this
/// build rather than sending one that points at nobody's socket.
pub fn plugin_for_install(input: &RemoteInstallInput<'_>) -> Option<String> {
    let owner_uid = input.host.socket_owner_uid(input.remote_socket_path);
    let replacements = [
        ("__MORROW_SOCKET_PATH__", input.remote_socket_path),
        ("__MORROW_REMOTE_HOST_ID__", input.host.id.as_str()),
        ("__MORROW_REMOTE_HOST_NAME__", input.host.name.as_str()),
        ("__MORROW_SOCKET_OWNER_UID__", owner_uid.as_str()),
    ];
    let mut configured = input.plugin_source.to_owned();
    for (placeholder, value) in replacements {
        let quoted = format!("\"{placeholder}\"");
        if !configured.contains(&quoted) {
            return None;
        }
        configured = configured.replace(&quoted, &quoted_literal(value));
    }
    Some(configured)
}

/// Build every file the remote install writes, using the local installer's
/// rules for each format.
pub fn plan(input: &RemoteInstallInput<'_>) -> RemoteConfigPlan {
    let bridge = HOOK_COMMAND.to_owned();
    let mut writes = vec![
        RemoteFileWrite {
            path: HOOK_PATH.into(),
            contents: input.hook_source.to_owned(),
            mode: 0o755,
        },
    ];
    let mut names = Vec::new();
    let mut skipped = Vec::new();

    if let Some(plugin) = plugin_for_install(input) {
        writes.push(RemoteFileWrite {
            path: PLUGIN_PATH.into(),
            contents: plugin,
            mode: 0o644,
        });
    } else {
        skipped.push("OpenCode plugin (placeholder mismatch)".into());
    }

    if let Some(outcome) = claude_hooks::install(
        get(input.existing, ".claude/settings.json"),
        &bridge,
        Some("0.4.4"),
    ) {
        writes.push(RemoteFileWrite {
            path: ".claude/settings.json".into(),
            contents: outcome.text,
            mode: 0o644,
        });
        names.push("Claude".into());
    }
    for (source, name) in [("qoder", "Qoder"), ("codebuddy", "CodeBuddy"), ("codex", "Codex")] {
        let Some(target) = targets::target_for(source) else {
            skipped.push(name.to_owned());
            continue;
        };
        let path = target.path.to_owned();
        if let Some(outcome) = targets::install(target, get(input.existing, &path), &bridge) {
            writes.push(RemoteFileWrite {
                path,
                contents: outcome.text,
                mode: 0o644,
            });
            names.push(name.to_owned());
        }
    }
    if let Some(text) = codex_config::enabling_hooks(get(input.existing, ".codex/config.toml").unwrap_or("")) {
        writes.push(RemoteFileWrite {
            path: ".codex/config.toml".into(),
            contents: text,
            mode: 0o644,
        });
    }
    for source in yaml_hooks::SOURCES {
        let path = source.path.to_owned();
        match yaml_hooks::install(source, get(input.existing, &path).unwrap_or(""), &bridge) {
            Ok(text) => {
                writes.push(RemoteFileWrite {
                    path,
                    contents: text,
                    mode: 0o644,
                });
                names.push(source.name.to_owned());
            }
            Err(_) => skipped.push(source.name.to_owned()),
        }
    }
    let opencode_path = if input.existing.contains_key(".config/opencode/opencode.jsonc") {
        ".config/opencode/opencode.jsonc"
    } else {
        ".config/opencode/opencode.json"
    };
    if let Some(text) = opencode::merge_plugin_ref(
        get(input.existing, opencode_path),
        "~/.codeisland/codeisland-opencode-remote.js",
    ) {
        writes.push(RemoteFileWrite {
            path: opencode_path.into(),
            contents: text,
            mode: 0o644,
        });
        names.push("OpenCode".into());
    }

    let summary = if names.is_empty() {
        "No remote hooks installed".to_owned()
    } else {
        format!("{} remote hooks installed", names.join("/"))
    };
    RemoteConfigPlan {
        writes,
        skipped,
        summary,
    }
}

/// A python3 program that writes the planned bytes under the remote home.
/// Base64 carries the contents, so no config byte is read by a shell.
pub fn install_script(plan: &RemoteConfigPlan) -> String {
    let mut out = String::from(
        "import base64, os, pathlib\n\nfiles = [\n",
    );
    for write in &plan.writes {
        let encoded = base64::engine::general_purpose::STANDARD.encode(write.contents.as_bytes());
        out.push_str(&format!(
            "    ({}, {}, {}),\n",
            quoted_literal(&write.path),
            quoted_literal(&encoded),
            write.mode,
        ));
    }
    out.push_str(
        "]\n\nfor path, encoded, mode in files:\n\
         \x20   target = pathlib.Path.home() / path\n\
         \x20   target.parent.mkdir(parents=True, exist_ok=True)\n\
         \x20   target.write_bytes(base64.b64decode(encoded))\n\
         \x20   os.chmod(target, mode)\n\
         \x20   print(target)\n",
    );
    out
}

/// Read the planned config files on the host as one JSON answer line.
pub fn snapshot_script() -> String {
    let paths = config_paths()
        .iter()
        .map(|path| quoted_literal(path))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "import json, pathlib\n\npaths = [{paths}]\nfiles = {{}}\nfor path in paths:\n\
         \x20   target = pathlib.Path.home() / path\n\
         \x20   try:\n\
         \x20       files[path] = target.read_text(encoding='utf-8')\n\
         \x20   except OSError:\n\
         \x20       pass\nprint({} + json.dumps(files))\n",
        quoted_literal(CONFIG_MARKER)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_answer_is_parsed_without_the_login_banner() {
        let answer = format!(
            "Last login: today\n{CONFIG_MARKER}{{\".claude/settings.json\": \"// keep\\n{{}}\"}}"
        );
        let files = config_snapshot(&answer);
        assert_eq!(files.get(".claude/settings.json").map(String::as_str), Some("// keep\n{}"));
    }
}
