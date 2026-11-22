//! The panel window (GLOSSARY.md: Panel, Tight-fit window): frameless,
//! transparent, always on top, out of the taskbar, never focused, and
//! exactly as big as the island, at the top centre of the primary monitor.
//! Clicks outside the island land on the app underneath because the window
//! is not there; no click-through API is used.

use tauri::{
    App, Manager, PhysicalPosition, PhysicalSize, PhysicalUnit, PixelUnit, Runtime, State,
    WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowSizeConstraints,
};

use crate::position::{self, MonitorGeometry};

pub const LABEL: &str = "island";

/// The collapsed pill before the frontend has measured it, in logical
/// pixels: about the macOS collapsed island on a screen without a notch
/// (`NotchPanelView.effectiveNotchHeight`, 34 pt).
const COLLAPSED: (f64, f64) = (240.0, 34.0);

/// `MORROW_SMOKE=1`: print each fit and each card's buttons on stdout for
/// linux/ci/smoke-x11.sh and linux/ci/e2e1.sh.
pub struct Smoke(pub bool);

/// Whether the island is a layer surface or an ordinary always-on-top window,
/// and the sentence the UI shows when it is the latter (#109).
///
/// On a compositor with `wlr-layer-shell` this *is* the seam a layer surface
/// would be created at: tao upstream closed both PRs (plan §6.4), so a build
/// that anchors needs the pinned patch, and until that decision is made the
/// window path stays and the reason is reported instead of guessed at.
#[tauri::command]
pub fn anchoring() -> morrow_platform::compositor::Anchoring {
    morrow_platform::compositor::current()
}

pub fn create(app: &App) -> tauri::Result<()> {
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::default())
        .title("Morrow")
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        // tao: GTK accept-focus false, so the window manager never gives it
        // focus, when it maps or when it is clicked (plan §6.2).
        .focusable(false)
        .focused(false)
        // GTK keeps a non-resizable window at its natural size and ignores
        // resizes, so the window is resizable and `fit` pins it instead:
        // minimum = maximum = the island, which the user cannot resize.
        .resizable(true)
        .inner_size(COLLAPSED.0, COLLAPSED.1)
        .visible(false)
        .build()?;
    // Until the frontend reports the chosen shape, start with the old pill.
    fit(
        &window,
        &app.state::<position::Store>(),
        app.state::<Smoke>().0,
    )?;
    // A panel without its Windows hooks still shows; it only behaves like an
    // ordinary window, which is worth a log line rather than no island at all.
    // Where the island lives on this desktop, said once at startup so a log
    // answers "why is it not above my fullscreen app?" (#109).
    if let Some(reason) = morrow_platform::compositor::current().reason() {
        println!("morrow: {reason}");
    }
    #[cfg(windows)]
    if let Err(error) = crate::windows_panel::install(&window) {
        eprintln!("morrow: the panel's Windows hooks were not installed: {error}");
    }
    window.show()
}

/// The frontend's measure of the island, in logical pixels. `radii` is the
/// real hit-test shape sent with the size, so Windows never answers with the
/// old single `h / 2` radius; `shoulders` is variant C's cut-out width. Both
/// are optional for older callers, which keep the pill. #196: a stored offset
/// survives this resize — `fit` no longer re-centres unconditionally.
#[tauri::command]
pub fn set_island_size(
    window: WebviewWindow,
    smoke: State<'_, Smoke>,
    position: State<'_, position::Store>,
    width: f64,
    height: f64,
    radii: Option<morrow_platform::panel::Radii>,
    shoulders: Option<i32>,
) -> Result<(), String> {
    if !(width.is_finite() && height.is_finite() && width >= 1.0 && height >= 1.0) {
        return Err(format!("not an island size: {width}×{height}"));
    }
    let radii = radii.unwrap_or_else(|| {
        morrow_platform::panel::Radii::uniform(
            (height / 2.0).round().clamp(0.0, i32::MAX as f64) as i32
        )
    });
    position.set_panel((width, height), radii, shoulders.unwrap_or(0));
    fit(&window, &position, smoke.0).map_err(|e| e.to_string())
}

/// #196: one pointer move of the collapsed bar, as a logical-pixel offset from
/// top-centre. Updates the live offset and moves the window; the file is
/// written once, by `commit_island_position`, when the pointer is released.
#[tauri::command]
pub fn move_island(
    window: WebviewWindow,
    smoke: State<'_, Smoke>,
    position: State<'_, position::Store>,
    x: f64,
    y: f64,
) -> Result<(), String> {
    let offset = position::Offset { x, y };
    if !offset.is_finite() {
        return Err(format!("not an island offset: {x},{y}"));
    }
    if let Some((_, key)) = current(&window).map_err(|e| e.to_string())? {
        position.set(&key, offset);
    }
    place(&window, &position, smoke.0)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Persist the live offset for this monitor.
#[tauri::command]
pub fn commit_island_position(position: State<'_, position::Store>) -> Result<(), String> {
    position.save().map_err(|e| e.to_string())
}

/// Tray's "Reset island position", also callable from the frontend.
#[tauri::command]
pub fn reset_island_position(
    window: WebviewWindow,
    smoke: State<'_, Smoke>,
    position: State<'_, position::Store>,
) -> Result<(), String> {
    reset(&window, &position, smoke.0)
}

/// Shared by the command and the tray menu: forget this monitor's offset and
/// fit again, so the island returns to the top centre.
pub fn reset<R: Runtime>(
    window: &WebviewWindow<R>,
    position: &position::Store,
    smoke: bool,
) -> Result<(), String> {
    if let Some((_, key)) = current(window).map_err(|e| e.to_string())? {
        position.remove(&key);
        position.save().map_err(|e| e.to_string())?;
    }
    fit(window, position, smoke).map_err(|e| e.to_string())
}

/// Hide the island while a fullscreen app is in front, show it again after
/// (#197). The tight-fit window owns the visibility; the DOM stays laid out so
/// `set_island_size` and the card rectangles keep their meaning.
#[tauri::command]
pub fn island_visible(window: WebviewWindow, visible: bool) -> Result<(), String> {
    if visible {
        window.show().map_err(|e| e.to_string())
    } else {
        window.hide().map_err(|e| e.to_string())
    }
}

/// A rectangle in the window, in logical pixels.
#[derive(serde::Deserialize)]
pub struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// A Question card opened or closed: on Windows the panel takes keyboard focus
/// for as long as it is open, and gives it back when it closes (#106). On other
/// platforms the window can always be focused, so this is a no-op.
#[tauri::command]
pub fn island_question_focus(window: WebviewWindow, open: bool) -> Result<(), String> {
    #[cfg(windows)]
    return crate::windows_panel::set_question_focus(&window, open);
    #[cfg(not(windows))]
    {
        let _ = (window, open);
        Ok(())
    }
}

/// The header's second power click within three seconds (#182). The frontend
/// owns the confirmation; this command only performs the exit.
#[tauri::command]
pub fn quit(app: tauri::AppHandle) {
    app.exit(0);
}

/// The frontend laid out card `id` and fitted the window to it. Smoke mode
/// prints the Approve and Deny buttons in window pixels and the time, so
/// the E2E-1 gate clicks them and times the card. Otherwise nothing.
#[tauri::command]
pub fn smoke_card(
    window: WebviewWindow,
    smoke: State<'_, Smoke>,
    id: u64,
    approve: Rect,
    deny: Rect,
) {
    if !smoke.0 {
        return;
    }
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let scale = window.scale_factor().unwrap_or(1.0);
    let px = |r: &Rect| {
        let p = |v: f64| (v * scale).round() as i64;
        format!("{},{},{},{}", p(r.x), p(r.y), p(r.width), p(r.height))
    };
    println!(
        "morrow-smoke: card id={id} approve={} deny={} at={at}",
        px(&approve),
        px(&deny)
    );
}

/// The monitor the island is fitted to, with the key its offset is stored
/// under. Primary first, current monitor as the fallback (#182's rule).
fn current<R: Runtime>(
    window: &WebviewWindow<R>,
) -> tauri::Result<Option<(MonitorGeometry, String)>> {
    let monitor = match window.primary_monitor()? {
        Some(monitor) => monitor,
        None => match window.current_monitor()? {
            Some(monitor) => monitor,
            None => return Ok(None),
        },
    };
    let geometry = MonitorGeometry::new(&monitor);
    let key = geometry.key(monitor.name().map(String::as_str));
    Ok(Some((geometry, key)))
}

/// Pin the size to the island and keep whatever offset this monitor has. A
/// layout change must never undo a drag (#196).
fn fit<R: Runtime>(
    window: &WebviewWindow<R>,
    position: &position::Store,
    smoke: bool,
) -> tauri::Result<()> {
    let panel = position.panel();
    let Some((geometry, _)) = current(window)? else {
        // No monitor to place on: keep the size, leave the position.
        return window.set_size(tauri::LogicalSize::new(panel.size.0, panel.size.1));
    };
    let to_px = |logical: f64, max: u32| ((logical * geometry.scale).ceil() as u32).clamp(1, max);
    let w = to_px(panel.size.0, geometry.size.0);
    let h = to_px(panel.size.1, geometry.size.1);
    let (pw, ph) = (pixels(w), pixels(h));
    window.set_size_constraints(WindowSizeConstraints {
        min_width: Some(pw),
        min_height: Some(ph),
        max_width: Some(pw),
        max_height: Some(ph),
    })?;
    window.set_size(PhysicalSize::new(w, h))?;
    let (x, y) = place(window, position, false)?;
    if smoke {
        println!("morrow-smoke: fit x={x} y={y} w={w} h={h}");
    }
    Ok(())
}

/// Move the window to home + the monitor's offset, without resizing it. This
/// is the drag's hot path: one `set_position` per pointer move.
fn place<R: Runtime>(
    window: &WebviewWindow<R>,
    position: &position::Store,
    smoke: bool,
) -> tauri::Result<(i32, i32)> {
    let Some((geometry, key)) = current(window)? else {
        return Ok((0, 0));
    };
    let panel = position.panel();
    let to_px = |logical: f64, max: u32| ((logical * geometry.scale).ceil() as u32).clamp(1, max);
    let w = to_px(panel.size.0, geometry.size.0);
    let h = to_px(panel.size.1, geometry.size.1);
    let (x, y) = position::place(&geometry, w as i32, h as i32, position.get(&key));
    window.set_position(PhysicalPosition::new(x, y))?;
    // Windows hit-tests the shape itself (#106, #182): the rectangle has to
    // travel with the island, and logical radii scale with the monitor.
    #[cfg(windows)]
    {
        let scale = geometry.scale;
        let px = |logical: i32| {
            i32::try_from(((logical as f64) * scale).ceil() as u32).unwrap_or(i32::MAX)
        };
        let size = PhysicalSize::new(
            (panel.size.0 * scale).ceil() as u32,
            (panel.size.1 * scale).ceil() as u32,
        );
        crate::windows_panel::set_panel(morrow_platform::panel::Panel {
            window: morrow_platform::panel::Rect::new(x, y, size.width as i32, size.height as i32),
            island: morrow_platform::panel::Rect::new(0, 0, size.width as i32, size.height as i32),
            radii: morrow_platform::panel::Radii::new(
                px(panel.radii.top_left),
                px(panel.radii.top_right),
                px(panel.radii.bottom_left),
                px(panel.radii.bottom_right),
            ),
            shoulders: px(panel.shoulders),
        });
    }
    #[cfg(not(windows))]
    let _ = (geometry, panel.radii, panel.shoulders);
    if smoke {
        println!("morrow-smoke: drag x={x} y={y}");
    }
    Ok((x, y))
}

fn pixels(px: u32) -> PixelUnit {
    PixelUnit::Physical(PhysicalUnit::new(i32::try_from(px).unwrap_or(i32::MAX)))
}
