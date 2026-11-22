//! #196: the island's position, remembered per monitor.
//!
//! The frontend sends a drag as a logical-pixel offset from top-centre. This
//! module keys that offset to the monitor, scales it to physical pixels when
//! the window is fitted, and stores the whole map under one settings key. The
//! settings writer keeps every other key — including a hand-edit — intact.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::json;

/// The one top-level key in `~/.codeisland/settings.json` this module owns.
pub const SETTINGS_KEY: &str = "islandPosition";

/// An offset from home, in logical pixels: independent of the monitor's DPI.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Offset {
    pub x: f64,
    pub y: f64,
}

impl Offset {
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

/// A monitor in physical pixels, with its scale factor for `Offset` → pixels.
#[derive(Clone, Copy, Debug)]
pub struct MonitorGeometry {
    pub origin: (i32, i32),
    pub size: (u32, u32),
    pub scale: f64,
}

impl MonitorGeometry {
    pub fn new(monitor: &tauri::Monitor) -> Self {
        Self {
            origin: (monitor.position().x, monitor.position().y),
            size: (monitor.size().width, monitor.size().height),
            scale: monitor.scale_factor(),
        }
    }

    pub fn key(&self, name: Option<&str>) -> String {
        monitor_key(name, self.origin, self.size)
    }
}

/// The monitor's name is stable when the OS gives one; origin is the fallback.
/// The size disambiguates two monitors with the same name at different DPIs.
pub fn monitor_key(name: Option<&str>, origin: (i32, i32), size: (u32, u32)) -> String {
    let id = name
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{}x{}", origin.0, origin.1));
    format!("{id}@{}x{}", size.0, size.1)
}

/// Top-centre of the monitor for an island of `w × h` physical pixels.
pub fn home(geo: &MonitorGeometry, w: i32, _h: i32) -> (i32, i32) {
    (geo.origin.0 + (geo.size.0 as i32 - w) / 2, geo.origin.1)
}

/// Home plus the scaled offset, kept partly on the monitor: at least 40
/// physical px of the island must remain reachable, and it cannot rise above
/// the top edge or sink below the bottom edge.
pub fn place(geo: &MonitorGeometry, w: i32, h: i32, offset: Offset) -> (i32, i32) {
    let (hx, hy) = home(geo, w, h);
    let scale = if geo.scale.is_finite() && geo.scale > 0.0 {
        geo.scale
    } else {
        1.0
    };
    let x = hx + (offset.x * scale).round() as i32;
    let y = hy + (offset.y * scale).round() as i32;
    let margin = (40.0 * scale).round() as i32;
    let min_x = geo.origin.0 + margin - w;
    let max_x = geo.origin.0 + geo.size.0 as i32 - margin;
    let min_y = geo.origin.1;
    let max_y = geo.origin.1 + geo.size.1 as i32 - h.min(geo.size.1 as i32);
    (
        x.clamp(min_x.min(max_x), max_x),
        y.clamp(min_y, max_y.max(min_y)),
    )
}

/// The last shape the frontend reported. A move needs no resize, and Windows
/// keeps its hit-test rectangle when the island is dragged.
#[derive(Clone, Copy)]
pub struct Panel {
    pub size: (f64, f64),
    pub radii: morrow_platform::panel::Radii,
    pub shoulders: i32,
}

impl Default for Panel {
    fn default() -> Self {
        Self {
            size: (240.0, 34.0),
            radii: morrow_platform::panel::Radii::uniform(17),
            shoulders: 0,
        }
    }
}

/// The live offsets plus the last shape the frontend reported.
pub struct Store {
    dir: PathBuf,
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    offsets: HashMap<String, Offset>,
    panel: Option<Panel>,
}

impl Store {
    pub fn new(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
            inner: Mutex::new(Inner {
                offsets: read_offsets(dir),
                panel: None,
            }),
        }
    }

    pub fn get(&self, key: &str) -> Offset {
        self.lock().offsets.get(key).copied().unwrap_or_default()
    }

    pub fn set(&self, key: &str, offset: Offset) {
        self.lock().offsets.insert(key.to_owned(), offset);
    }

    pub fn panel(&self) -> Panel {
        self.lock().panel.unwrap_or_default()
    }

    pub fn set_panel(
        &self,
        size: (f64, f64),
        radii: morrow_platform::panel::Radii,
        shoulders: i32,
    ) {
        self.lock().panel = Some(Panel {
            size,
            radii,
            shoulders,
        });
    }

    /// Remove an entry. Persistence is a separate, explicit step so a drag
    /// writes the file once, on release, and not on every pointer move.
    pub fn remove(&self, key: &str) -> bool {
        self.lock().offsets.remove(key).is_some()
    }

    pub fn save(&self) -> io::Result<()> {
        let offsets = self.lock().offsets.clone();
        let value = serde_json::to_value(offsets).map_err(io::Error::other)?;
        morrow_platform::settings::set(&self.dir, SETTINGS_KEY, &value).map(|_| ())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|error| error.into_inner())
    }
}

fn read_offsets(dir: &Path) -> HashMap<String, Offset> {
    let Ok(document) = morrow_platform::settings::load(dir) else {
        return HashMap::new();
    };
    serde_json::from_value(
        document
            .get(SETTINGS_KEY)
            .cloned()
            .unwrap_or_else(|| json!({})),
    )
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;

    use super::*;

    fn temp(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("morrow-position-{tag}-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn desktop() -> MonitorGeometry {
        MonitorGeometry {
            origin: (0, 0),
            size: (1920, 1080),
            scale: 1.0,
        }
    }

    #[test]
    fn home_is_top_centre_of_the_monitor() {
        assert_eq!(home(&desktop(), 680, 400), (620, 0));
        let second = MonitorGeometry {
            origin: (1920, 0),
            ..desktop()
        };
        assert_eq!(home(&second, 240, 34), (2760, 0));
    }

    #[test]
    fn an_offset_is_scaled_from_logical_to_physical_pixels() {
        // A 1920×1080 logical desktop at 2× is 3840×2160 physical; the island
        // is 240×34 logical, or 480×68 physical here.
        let hidpi = MonitorGeometry {
            size: (3840, 2160),
            scale: 2.0,
            ..desktop()
        };
        assert_eq!(
            place(&hidpi, 480, 68, Offset { x: 100.0, y: 50.0 }),
            (1880, 100)
        );
    }

    #[test]
    fn a_dragged_island_stays_partly_on_its_monitor() {
        let far = place(
            &desktop(),
            240,
            34,
            Offset {
                x: 10_000.0,
                y: 10_000.0,
            },
        );
        assert_eq!(far, (1880, 1046));
        let near = place(
            &desktop(),
            240,
            34,
            Offset {
                x: -10_000.0,
                y: -10_000.0,
            },
        );
        assert_eq!(near, (-200, 0));
    }

    #[test]
    fn the_monitor_key_survives_a_restart() {
        assert_eq!(
            monitor_key(Some("eDP-1"), (0, 0), (1920, 1080)),
            "eDP-1@1920x1080"
        );
        // A monitor the OS does not name is keyed by where it sits.
        assert_eq!(
            monitor_key(None, (1920, 0), (1920, 1080)),
            "1920x0@1920x1080"
        );
    }

    #[test]
    fn offsets_survive_a_restart_and_leave_other_settings_alone() {
        let dir = temp("roundtrip");
        let store = Store::new(&dir);
        store.set("eDP-1@1920x1080", Offset { x: 120.0, y: 40.0 });
        store.set("HDMI-1@2560x1440", Offset { x: -80.0, y: 16.0 });
        store.save().unwrap();

        morrow_platform::settings::set(&dir, "theme", &json!("dark")).unwrap();
        let reopened = Store::new(&dir);
        assert_eq!(
            reopened.get("eDP-1@1920x1080"),
            Offset { x: 120.0, y: 40.0 }
        );
        assert_eq!(
            reopened.get("HDMI-1@2560x1440"),
            Offset { x: -80.0, y: 16.0 }
        );
        assert_eq!(reopened.get("other"), Offset::default());
        let text = fs::read_to_string(morrow_platform::settings::path(&dir)).unwrap();
        assert!(text.contains("\"theme\": \"dark\""), "{text}");

        assert!(reopened.remove("eDP-1@1920x1080"));
        reopened.save().unwrap();
        assert_eq!(Store::new(&dir).get("eDP-1@1920x1080"), Offset::default());
        assert_eq!(
            Store::new(&dir).get("HDMI-1@2560x1440"),
            Offset { x: -80.0, y: 16.0 }
        );
    }
}
