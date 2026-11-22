//! The panel's shape (#106): which points are island and which are window-only,
//! because the corners are round and the window is not.

use morrow_platform::panel::{Panel, Radii, Rect, contains_rounded, contains_shape};

fn pill() -> Rect {
    // The collapsed island: 240 × 34, radius half the height.
    Rect::new(0, 0, 240, 34)
}

#[test]
fn the_middle_of_a_pill_is_island() {
    assert!(contains_rounded(pill(), 17, 120, 17));
    assert!(
        contains_rounded(pill(), 17, 0, 17),
        "the straight left edge"
    );
    assert!(
        contains_rounded(pill(), 17, 17, 5),
        "top edge, past the arc"
    );
}

#[test]
fn a_corner_of_the_window_is_not_the_island() {
    // The window's own top-left pixel: inside the box, outside the arc.
    assert!(!contains_rounded(pill(), 17, 0, 0));
    assert!(!contains_rounded(pill(), 17, 3, 1));
    assert!(contains_rounded(pill(), 17, 6, 6), "inside the arc");
    // The same, mirrored to the other three corners.
    assert!(!contains_rounded(pill(), 17, 239, 0));
    assert!(!contains_rounded(pill(), 17, 239, 33));
    assert!(!contains_rounded(pill(), 17, 0, 33));
}

#[test]
fn outside_the_box_is_never_island() {
    assert!(!contains_rounded(pill(), 17, -1, 17));
    assert!(
        !contains_rounded(pill(), 17, 240, 17),
        "right edge is exclusive"
    );
    assert!(!contains_rounded(pill(), 17, 120, 34));
}

#[test]
fn a_square_window_ignores_the_radius() {
    assert!(contains_rounded(Rect::new(0, 0, 10, 10), 0, 0, 0));
    // A radius larger than half the side is clamped to half, never borrowed
    // from the other dimension.
    assert!(contains_rounded(Rect::new(0, 0, 10, 10), 99, 5, 5));
}

#[test]
fn a_panel_offset_in_its_window_maps_the_point() {
    let panel = Panel {
        window: Rect::new(100, 200, 240, 34),
        island: Rect::new(0, 0, 240, 34),
        radii: Radii::uniform(17),
        shoulders: 0,
    };
    assert!(panel.is_island(120, 17), "island coordinates");
    assert!(!panel.is_island(0, 0), "the window's corner");
}

#[test]
fn an_island_smaller_than_its_window_leaves_a_margin() {
    // The expanded card sits inside a window that has room for the shadow.
    let panel = Panel {
        window: Rect::new(100, 200, 400, 200),
        island: Rect::new(20, 10, 360, 180),
        radii: Radii::uniform(12),
        shoulders: 0,
    };
    assert!(panel.is_island(300, 100), "on the card");
    assert!(!panel.is_island(5, 5), "in the shadow margin");
    assert!(
        !panel.is_island(395, 100),
        "past the card, inside the window"
    );
}

// #182: the old single `h / 2` radius clamps to 200 pt on a 680 × 400 panel,
// so a click in a top corner fell through to the app underneath. The frontend
// now sends the real shape with the size.

#[test]
fn the_open_panel_has_a_square_top_and_24pt_bottom() {
    let rect = Rect::new(0, 0, 680, 400);
    let radii = Radii::new(0, 0, 24, 24);
    assert!(contains_shape(rect, radii, 0, 0, 0), "top-left corner");
    assert!(contains_shape(rect, radii, 0, 679, 0), "top-right corner");
    assert!(contains_shape(rect, radii, 0, 0, 375), "bottom-left arc");
    assert!(
        contains_shape(rect, radii, 0, 679, 375),
        "bottom-right arc"
    );
    assert!(contains_shape(rect, radii, 0, 340, 399), "bottom edge");
    assert!(!contains_shape(rect, radii, 0, 0, 399), "outside the arc");
    assert!(
        !contains_shape(rect, radii, 0, 679, 399),
        "outside the other arc"
    );
    assert!(!contains_shape(rect, radii, 0, -1, 20), "outside the window");
}

#[test]
fn the_shoulder_variant_excludes_its_cutouts() {
    let rect = Rect::new(0, 0, 680, 400);
    let radii = Radii::new(0, 0, 24, 24);
    assert!(!contains_shape(rect, radii, 14, 2, 2), "left shoulder cut-out");
    assert!(
        !contains_shape(rect, radii, 14, 677, 2),
        "right shoulder cut-out"
    );
    assert!(contains_shape(rect, radii, 14, 20, 2), "between shoulders");
}

#[test]
fn the_shape_travels_with_the_size_in_the_panel() {
    let panel = Panel {
        window: Rect::new(100, 200, 680, 400),
        island: Rect::new(0, 0, 680, 400),
        radii: Radii::new(0, 0, 24, 24),
        shoulders: 0,
    };
    assert!(panel.is_island(0, 0));
    assert!(panel.is_island(679, 0));
    assert!(!panel.is_island(0, 399));
}
