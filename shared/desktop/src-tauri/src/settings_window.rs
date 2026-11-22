//! The separate Settings window (#199, W-009/L-012): a normal, focusable,
//! decorated window. It is created on the first open from the gear or the tray
//! and reused while it is alive; closing it and opening again builds a new one.
//!
//! The island stays frameless and non-focusable — nothing here touches it or
//! its E2E-1 fit chain.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub const LABEL: &str = "settings";
pub const URL: &str = "settings.html";

/// Show Settings, creating it on first use.
#[tauri::command]
pub fn open_settings(app: AppHandle) -> Result<(), String> {
    let window = match app.get_webview_window(LABEL) {
        Some(window) => window,
        None => WebviewWindowBuilder::new(&app, LABEL, WebviewUrl::App(URL.into()))
            .title("Morrow Settings")
            .decorations(true)
            .focusable(true)
            .focused(true)
            .resizable(true)
            .inner_size(760.0, 470.0)
            .min_inner_size(560.0, 400.0)
            .center()
            .build()
            .map_err(|error| error.to_string())?,
    };
    let _ = window.unminimize();
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())?;
    Ok(())
}
