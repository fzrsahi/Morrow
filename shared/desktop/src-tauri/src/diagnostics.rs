//! #465 — the diagnostics export the About page offers.
//!
//! Swift writes a zip through an AppKit save panel and fills it with system
//! logs, crash reports and redacted CLI configs. This shell has no unified log
//! or crash-report folder and no save-panel plugin, so it writes one pretty
//! JSON bundle to a path the user types in Settings. What goes into the bundle
//! follows the same rule: settings and session structure, secrets replaced.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use tauri::State;

use crate::HooksSite;

/// The default filename Swift's save panel suggests, with `.json` instead of
/// `.zip` because there is no zip writer in the shell's dependency set.
pub fn suggested_name() -> String {
    format!("Morrow-Diagnostics-{}.json", stamp())
}

/// `yyyyMMdd-HHmmss` in UTC, the shape Swift's `DiagnosticsExporter.timestamp`
/// uses for its suggested filename.
fn stamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let iso = morrow_core::time::format_iso8601(morrow_core::reduce::Timestamp(seconds as f64));
    // `2026-11-25T18:19:56` → `20261125-181956`, Swift's `yyyyMMdd-HHmmss`.
    iso[..19]
        .replace('-', "")
        .replace('T', "-")
        .replace(':', "")
}

/// Build the bundle without touching the network or any OS log: only the
/// settings document and the persisted session rows are read.
fn collect(site: &HooksSite) -> Value {
    let settings = morrow_platform::settings::load(&site.paths.codeisland_dir).unwrap_or_default();
    let sessions = morrow_core::session_store::load(&crate::feed::sessions_directory(
        &site.paths.home,
    ))
    .into_iter()
    .filter_map(|row| serde_json::to_value(row).ok())
    .collect::<Vec<_>>();
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let metadata = json!({
        "exportedAt": morrow_core::time::format_iso8601(
            morrow_core::reduce::Timestamp(seconds as f64)
        ),
        "appVersion": env!("CARGO_PKG_VERSION"),
        "platform": std::env::consts::OS,
        "settingsPath": morrow_platform::settings::path(&site.paths.codeisland_dir),
        "sessionsSource": "persisted rows; live cards are not written to disk",
    });
    morrow_core::diagnostics::bundle(metadata, &settings, &sessions)
}

/// Where a typed destination ends up: a directory gets the suggested filename,
/// a file path is used as-is. A trailing slash is therefore still a directory.
fn resolve_destination(raw: &str) -> PathBuf {
    let path = PathBuf::from(raw.trim());
    if raw.trim().ends_with('/') || raw.trim().ends_with('\\') || path.is_dir() {
        path.join(suggested_name())
    } else {
        path
    }
}

/// Write the bundle. Returns the final path so About can show it and the user
/// can attach the right file to a report.
#[tauri::command]
pub fn diagnostics_export(site: State<'_, HooksSite>, destination: String) -> Result<PathBuf, String> {
    let destination = resolve_destination(&destination);
    if destination.as_os_str().is_empty() {
        return Err("choose a destination first".to_owned());
    }
    if let Some(parent) = destination.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let bundle = collect(&site);
    let text = serde_json::to_string_pretty(&bundle).map_err(|error| error.to_string())?;
    fs::write(&destination, text).map_err(|error| error.to_string())?;
    Ok(destination)
}

/// The suggested name, also shown in the Settings input as a placeholder.
#[tauri::command]
pub fn diagnostics_suggested_name() -> String {
    suggested_name()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_directory_gets_the_suggested_filename() {
        let path = resolve_destination("/tmp/morrow-diag/");
        assert_eq!(path.parent().map(Path::to_str), Some(Some("/tmp/morrow-diag")));
        assert!(path.file_name().and_then(|n| n.to_str()).unwrap().starts_with("Morrow-Diagnostics-"));
        assert!(path.file_name().and_then(|n| n.to_str()).unwrap().ends_with(".json"));
    }

    #[test]
    fn a_file_path_is_used_as_is() {
        let path = resolve_destination("  /tmp/report.json  ");
        assert_eq!(path, PathBuf::from("/tmp/report.json"));
    }

    #[test]
    fn the_suggested_stamp_has_the_swift_shape() {
        let name = suggested_name();
        let stamp = name
            .trim_start_matches("Morrow-Diagnostics-")
            .trim_end_matches(".json");
        assert_eq!(stamp.len(), 15, "{name}");
        assert!(stamp.chars().all(|c| c.is_ascii_digit() || c == '-'));
    }
}
