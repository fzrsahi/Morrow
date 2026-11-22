//! #200 — the anchoring mini-peek (canvas section #anchoring), its own small
//! window so the island's tight-fit/E2E-1 window is never resized or
//! re-positioned by a notice. Created once in `setup`, only when the anchoring
//! probe says the panel is an ordinary window; the frontend owns the 5 s
//! lifetime and the "keep it while hovered" rule.

use morrow_platform::compositor::Anchoring;
use tauri::{App, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const LABEL: &str = "peek";
pub const URL: &str = "peek.html";

/// 440 × 100 pt from the canvas (#180 tokens; the #182 window rule keeps the
/// OS window pinned to the content instead of resizing on every layout frame).
const SIZE: (f64, f64) = (440.0, 100.0);
/// Just below the collapsed island pill (34 pt).
const ISLAND_HEIGHT: f64 = 34.0;
const GAP: f64 = 8.0;

/// X11 and a compositor with wlr-layer-shell report `Available` and show no
/// notice at all; an unknown Wayland session, GNOME or an AppImage do.
pub fn should_show(anchoring: &Anchoring) -> bool {
    !anchoring.is_available()
}

/// Create the peek at launch when there is something to say. "Once per launch"
/// is this single call in `setup`; there is no path that builds it again.
pub fn maybe_show(app: &App) -> tauri::Result<()> {
    if !should_show(&crate::island::anchoring()) {
        return Ok(());
    }
    let (x, y) = position(app);
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App(URL.into()))
        .title("Morrow")
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        // The island never takes focus; neither does its notice.
        .focusable(false)
        .focused(false)
        .resizable(true)
        .inner_size(SIZE.0, SIZE.1)
        .min_inner_size(SIZE.0, SIZE.1)
        .max_inner_size(SIZE.0, SIZE.1)
        .position(x, y)
        .visible(false)
        .build()?;
    window.show()
}

/// Top-centre on the primary monitor, under the island pill. Logical pixels,
/// as the builder's `position` and `inner_size` are.
fn position(app: &App) -> (f64, f64) {
    let Some(monitor) = app.primary_monitor().ok().flatten() else {
        return (0.0, ISLAND_HEIGHT + GAP);
    };
    let logical_width = monitor.size().width as f64 / monitor.scale_factor();
    (((logical_width - SIZE.0) / 2.0).max(0.0), ISLAND_HEIGHT + GAP)
}

/// The frontend dismisses the peek after its lifetime; a custom command is
/// used because the capability file grants `core:default` to `island` only.
#[tauri::command]
pub fn close_peek(window: WebviewWindow) -> Result<(), String> {
    window.close().map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn x11_and_layer_shell_show_nothing() {
        assert!(!should_show(&Anchoring::Available));
    }

    #[test]
    fn an_unavailable_compositor_shows_the_notice() {
        assert!(should_show(&Anchoring::Unavailable("reason".into())));
    }
}
