//! X11 readers for the pointer, fullscreen state and frontmost terminal (#399).
//!
//! The desktop already ships `xdotool` and `xprop`; asking them through
//! [`crate::runner::Runner`] keeps this crate free of an X11 library and keeps
//! every reader testable without a display. The parsers take injected stdout,
//! and each live call degrades to `None`/`false` when a tool is missing or the
//! machine is not on X11.
//!
//! Scope: app-level terminal visibility, the same question the Swift detector
//! answers in `liveIsTerminalFrontmost`. X11 cannot name the tab or tmux pane
//! behind a window, so per-session visibility stays a macOS-only feature.

use crate::runner::Runner;

/// Terminal window classes X11 reports through `WM_CLASS`. One table, so the
/// reader and its tests cannot disagree about what counts as a terminal.
const TERMINAL_CLASSES: &[&str] = &[
    "alacritty",
    "contour",
    "foot",
    "footex",
    "ghostty",
    "gnome-terminal",
    "gnome-terminal-server",
    "konsole",
    "kitty",
    "lxterminal",
    "mate-terminal",
    "qterminal",
    "rxvt",
    "rxvt-unicode",
    "st",
    "terminator",
    "tilix",
    "urxvt",
    "wezterm",
    "xterm",
    "xfce4-terminal",
];

/// Coordinates from `xdotool getmouselocation --shell`, or `None` if either
/// axis is missing or not an integer.
pub fn parse_pointer_location(output: &str) -> Option<(i32, i32)> {
    let (mut x, mut y) = (None, None);
    for line in output.lines() {
        let Some((key, value)) = line.trim().split_once('=') else {
            continue;
        };
        match key.trim() {
            "X" => x = Some(value.trim().parse::<i32>().ok()?),
            "Y" => y = Some(value.trim().parse::<i32>().ok()?),
            _ => {}
        }
    }
    Some((x?, y?))
}

/// A decimal window id from `xdotool getactivewindow`. Anything else is
/// rejected before it can become an `xprop` argument.
pub fn parse_window_id(output: &str) -> Option<String> {
    output.trim().parse::<u64>().ok().map(|id| id.to_string())
}

/// The first `WM_CLASS` value from `xprop -id <window> WM_CLASS`, lowercased.
/// xprop prints the instance first: `WM_CLASS(STRING) = "kitty", "kitty"`.
pub fn parse_wm_class(output: &str) -> Option<String> {
    let line = output
        .lines()
        .find(|line| line.trim_start().starts_with("WM_CLASS") && line.contains('='))?;
    let value = line.split_once('=')?.1;
    let class = value.split_once('"')?.1.split_once('"')?.0.trim();
    (!class.is_empty()).then(|| class.to_ascii_lowercase())
}

/// Whether `_NET_WM_STATE` lists `_NET_WM_STATE_FULLSCREEN` as a whole atom.
pub fn parse_fullscreen(output: &str) -> bool {
    output
        .lines()
        .filter(|line| line.trim_start().starts_with("_NET_WM_STATE"))
        .flat_map(|line| {
            line.split(|c: char| c == ',' || c.is_whitespace() || c == '=' || c == '(' || c == ')')
        })
        .any(|atom| atom == "_NET_WM_STATE_FULLSCREEN")
}

/// Whether a `WM_CLASS` instance names a known terminal app.
pub fn is_terminal_class(class: &str) -> bool {
    let class = class.trim();
    TERMINAL_CLASSES
        .iter()
        .any(|known| class.eq_ignore_ascii_case(known))
}

/// The pointer in screen pixels, or `None` when X11 will not say.
pub fn pointer_position(runner: &dyn Runner) -> Option<(i32, i32)> {
    let output = runner
        .run("xdotool", &["getmouselocation", "--shell"])
        .ok()?;
    parse_pointer_location(&output)
}

/// The window X11 reports as active, or `None` when there is none.
pub fn active_window(runner: &dyn Runner) -> Option<String> {
    let output = runner.run("xdotool", &["getactivewindow"]).ok()?;
    parse_window_id(&output)
}

/// The `WM_CLASS` instance of the active window, if xprop can read it.
pub fn terminal_class(runner: &dyn Runner) -> Option<String> {
    let window = active_window(runner)?;
    let output = runner.run("xprop", &["-id", &window, "WM_CLASS"]).ok()?;
    parse_wm_class(&output)
}

/// Whether a terminal app owns the active window. App-level only: no tab, pane
/// or remote session identity is claimed here.
pub fn terminal_is_frontmost(runner: &dyn Runner) -> bool {
    terminal_class(runner).is_some_and(|class| is_terminal_class(&class))
}

/// Whether the active window is fullscreen. An unreadable state is "no", so a
/// toolless session never hides the island.
pub fn fullscreen(runner: &dyn Runner) -> bool {
    let Some(window) = active_window(runner) else {
        return false;
    };
    let Ok(output) = runner.run("xprop", &["-id", &window, "_NET_WM_STATE"]) else {
        return false;
    };
    parse_fullscreen(&output)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake {
        answer: fn(&str, &[&str]) -> Result<String, String>,
    }

    impl Runner for Fake {
        fn run(&self, program: &str, args: &[&str]) -> Result<String, String> {
            (self.answer)(program, args)
        }
    }

    fn desktop(_program: &str, args: &[&str]) -> Result<String, String> {
        match args {
            ["getactivewindow"] => Ok("4194305\n".into()),
            ["-id", _, "WM_CLASS"] => Ok("WM_CLASS(STRING) = \"kitty\", \"kitty\"\n".into()),
            ["-id", _, "_NET_WM_STATE"] => {
                Ok("_NET_WM_STATE(ATOM) = _NET_WM_STATE_ABOVE, _NET_WM_STATE_FULLSCREEN\n".into())
            }
            _ => Err("unexpected tool call".into()),
        }
    }

    fn failed(_program: &str, _args: &[&str]) -> Result<String, String> {
        Err("xdotool: command not found".into())
    }

    #[test]
    fn pointer_output_is_parsed_from_shell_variables() {
        assert_eq!(
            parse_pointer_location("X=812\nY=-4\nSCREEN=0\nWINDOW=4194305\n"),
            Some((812, -4))
        );
        assert_eq!(parse_pointer_location("X=812\n"), None);
        assert_eq!(parse_pointer_location("X=\nY=4\n"), None);
        assert_eq!(parse_pointer_location("X=not-a-number\nY=4\n"), None);
        assert_eq!(parse_pointer_location(""), None);
    }

    #[test]
    fn window_id_is_a_decimal_number_only() {
        assert_eq!(parse_window_id("4194305\n"), Some("4194305".to_owned()));
        assert_eq!(parse_window_id("  42 "), Some("42".to_owned()));
        assert_eq!(parse_window_id("0x400005\n"), None);
        assert_eq!(parse_window_id("getactivewindow: no window\n"), None);
        assert_eq!(parse_window_id(""), None);
    }

    #[test]
    fn wm_class_reads_the_first_quoted_instance() {
        assert_eq!(
            parse_wm_class("WM_CLASS(STRING) = \"gnome-terminal\", \"Gnome-terminal\"\n"),
            Some("gnome-terminal".to_owned())
        );
        assert_eq!(
            parse_wm_class("WM_HINTS(WM_HINTS):\nWM_CLASS(STRING) = \"Kitty\", \"kitty\"\n"),
            Some("kitty".to_owned())
        );
        assert_eq!(parse_wm_class("WM_CLASS(STRING) = \n"), None);
        assert_eq!(
            parse_wm_class("_NET_WM_NAME(UTF8_STRING) = \"kitty\"\n"),
            None
        );
    }

    #[test]
    fn fullscreen_is_an_exact_atom_not_a_prefix() {
        assert!(parse_fullscreen(
            "_NET_WM_STATE(ATOM) = _NET_WM_STATE_FULLSCREEN\n"
        ));
        assert!(parse_fullscreen(
            "_NET_WM_STATE(ATOM) = _NET_WM_STATE_ABOVE, _NET_WM_STATE_FULLSCREEN\n"
        ));
        assert!(!parse_fullscreen(
            "_NET_WM_STATE(ATOM) = _NET_WM_STATE_ABOVE\n"
        ));
        assert!(!parse_fullscreen(
            "_NET_WM_STATE(ATOM) = _NET_WM_STATE_FULLSCREEN_FOO\n"
        ));
        assert!(!parse_fullscreen("WM_STATE(WM_STATE):\n"));
        assert!(!parse_fullscreen(""));
    }

    #[test]
    fn terminal_classes_come_from_one_table() {
        for class in [
            "kitty",
            "WezTerm",
            "gnome-terminal-server",
            "xfce4-terminal",
        ] {
            assert!(is_terminal_class(class), "{class}");
        }
        for class in ["firefox", "code", "_NET_WM_STATE_FULLSCREEN", ""] {
            assert!(!is_terminal_class(class), "{class}");
        }
    }

    #[test]
    fn live_readers_use_the_injected_runner() {
        let runner = Fake { answer: desktop };
        assert_eq!(active_window(&runner).as_deref(), Some("4194305"));
        assert_eq!(terminal_class(&runner).as_deref(), Some("kitty"));
        assert!(terminal_is_frontmost(&runner));
        assert!(fullscreen(&runner));
        // No X server is involved: a failed tool call is just an empty answer.
        let broken = Fake { answer: failed };
        assert_eq!(pointer_position(&broken), None);
        assert_eq!(active_window(&broken), None);
        assert_eq!(terminal_class(&broken), None);
        assert!(!terminal_is_frontmost(&broken));
        assert!(!fullscreen(&broken));
    }
}
