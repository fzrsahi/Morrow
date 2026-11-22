//! Where the panel is, and whether a point on screen is on it (#106, #182).
//!
//! Windows has to answer that question itself: the island's corners are round,
//! and a window rectangle cannot be round, so the last few pixels of each corner
//! are window but not island. Without this, a click aimed at the app *behind* a
//! corner is swallowed by an invisible part of the panel — the ticket's
//! "klik di luar island mengenai app di bawahnya" failing at the corners only,
//! which is exactly the kind of bug that survives a manual test.
//!
//! The radius used to be a single `h / 2` for all four corners. On the open
//! 680 × 400 panel that clamped to 200 pt and swallowed clicks in the square
//! top corners; the frontend now sends all four radii with the size, plus the
//! shoulder cut-outs of attachment variant C.

use serde::Deserialize;

/// A rectangle in one space (physical pixels, screen or window).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn right(self) -> i32 {
        self.x.saturating_add(self.width)
    }

    pub fn bottom(self) -> i32 {
        self.y.saturating_add(self.height)
    }

    /// Whether a point is inside, edges included.
    pub fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }
}

/// The corner radii of the island, in physical pixels. Sent by the frontend
/// with the size, so a pill, an edge tab and a shoulder tab all hit-test as
/// they are drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Radii {
    pub top_left: i32,
    pub top_right: i32,
    pub bottom_left: i32,
    pub bottom_right: i32,
}

impl Radii {
    pub fn new(top_left: i32, top_right: i32, bottom_left: i32, bottom_right: i32) -> Self {
        Self {
            top_left,
            top_right,
            bottom_left,
            bottom_right,
        }
    }

    pub fn uniform(radius: i32) -> Self {
        Self::new(radius, radius, radius, radius)
    }
}

/// Whether `(x, y)` is on a shape with per-corner radii and optional shoulder
/// cut-outs (variant C): inside the box, outside the cut-outs, and inside the
/// corner arcs when the point falls in a corner square.
///
/// Each radius is clamped to half the shorter side, so a caller cannot describe
/// a shape its own rectangle does not have.
pub fn contains_shape(rect: Rect, radii: Radii, shoulders: i32, x: i32, y: i32) -> bool {
    if !rect.contains(x, y) {
        return false;
    }
    if shoulders > 0
        && y < rect.y + shoulders
        && (x < rect.x + shoulders || x >= rect.right() - shoulders)
    {
        return false;
    }
    let max = rect.width.min(rect.height) / 2;
    let corners = [
        (
            x < rect.x + radii.top_left.clamp(0, max),
            y < rect.y + radii.top_left.clamp(0, max),
            radii.top_left.clamp(0, max),
            rect.x + radii.top_left.clamp(0, max),
            rect.y + radii.top_left.clamp(0, max),
        ),
        (
            x >= rect.right() - radii.top_right.clamp(0, max),
            y < rect.y + radii.top_right.clamp(0, max),
            radii.top_right.clamp(0, max),
            rect.right() - radii.top_right.clamp(0, max),
            rect.y + radii.top_right.clamp(0, max),
        ),
        (
            x < rect.x + radii.bottom_left.clamp(0, max),
            y >= rect.bottom() - radii.bottom_left.clamp(0, max),
            radii.bottom_left.clamp(0, max),
            rect.x + radii.bottom_left.clamp(0, max),
            rect.bottom() - radii.bottom_left.clamp(0, max),
        ),
        (
            x >= rect.right() - radii.bottom_right.clamp(0, max),
            y >= rect.bottom() - radii.bottom_right.clamp(0, max),
            radii.bottom_right.clamp(0, max),
            rect.right() - radii.bottom_right.clamp(0, max),
            rect.bottom() - radii.bottom_right.clamp(0, max),
        ),
    ];
    for (in_x, in_y, radius, cx, cy) in corners {
        if !(in_x && in_y) || radius == 0 {
            continue;
        }
        let (dx, dy) = (i64::from(x - cx), i64::from(y - cy));
        if dx * dx + dy * dy > i64::from(radius) * i64::from(radius) {
            return false;
        }
    }
    true
}

/// Backwards-compatible single-radius shape (a pill, and the old tests).
pub fn contains_rounded(rect: Rect, radius: i32, x: i32, y: i32) -> bool {
    contains_shape(rect, Radii::uniform(radius), 0, x, y)
}

/// Where the island sits inside its window, and its shape there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Panel {
    pub window: Rect,
    pub island: Rect,
    pub radii: Radii,
    pub shoulders: i32,
}

impl Panel {
    /// Whether a point in *window* coordinates is on the island. A point on the
    /// window but not the island is where a click must reach what is below.
    ///
    /// `island` is already in window coordinates; `window` is only here so a
    /// caller can say where the window is on screen without converting twice.
    pub fn is_island(self, x: i32, y: i32) -> bool {
        contains_shape(self.island, self.radii, self.shoulders, x, y)
    }
}
