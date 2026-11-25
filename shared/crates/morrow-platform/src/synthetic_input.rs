//! Synthetic key presses for Linux and Windows (#439).
//!
//! Swift posts Cmd+1...Cmd+9 to Warp's pid from `TerminalActivator.swift` to
//! jump to a tab. There is no Quartz on these OSes: Linux uses `xdotool`'s XTEST
//! injection and Windows uses PowerShell's `SendKeys`. Control is the portable
//! stand-in for Command, since it is the tab-switch modifier terminals use on
//! both platforms. Every call goes through [`Runner`] as an argument array, so
//! neither a key name nor user text is ever handed to a shell.

use crate::capability::{Capability, Os};
use crate::runner::Runner;

pub const XDOTOOL: &str = "xdotool";
pub const POWERSHELL: &str = "powershell.exe";

/// A key this adapter knows how to post. Swift only posts Warp's tab digits;
/// anything else is rejected rather than guessed at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    digit: u8,
}

impl Key {
    /// Parse the key names callers can ask for: the digits `1` to `9`.
    pub fn parse(name: &str) -> Result<Self, String> {
        let digit = match name {
            "1" => 1,
            "2" => 2,
            "3" => 3,
            "4" => 4,
            "5" => 5,
            "6" => 6,
            "7" => 7,
            "8" => 8,
            "9" => 9,
            _ => {
                return Err(format!(
                    "unknown key {name:?}: only digits 1 to 9 are supported"
                ));
            }
        };
        Ok(Self { digit })
    }
}

/// The xdotool keysym. Digits are their own keysym, so no translation table
/// beyond validation is needed.
pub fn xdotool_keysym(key: Key) -> &'static str {
    const KEYSYMS: [&str; 9] = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];
    KEYSYMS[usize::from(key.digit - 1)]
}

/// The SendKeys token. Digits are literal in SendKeys; the modifier is added
/// by [`press_key`].
pub fn sendkeys_token(key: Key) -> String {
    key.digit.to_string()
}

/// Escape the characters SendKeys treats as commands rather than text.
/// Each one becomes a braced literal: `{` is `{{}`, `}` is `{}}`, and so on.
pub fn escape_sendkeys(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '{' | '}' | '+' | '^' | '%' | '~' | '(' | ')' => {
                escaped.push('{');
                escaped.push(character);
                escaped.push('}');
            }
            _ => escaped.push(character),
        }
    }
    escaped
}

/// A PowerShell single-quoted literal ends at a doubled quote. Text goes into
/// that literal, never into a command line the shell can reinterpret.
fn powershell_literal(text: &str) -> String {
    text.replace('\'', "''")
}

fn sendkeys(runner: &dyn Runner, keys: &str) -> Result<(), String> {
    let script = format!(
        "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SendKeys]::SendWait('{}')",
        powershell_literal(keys)
    );
    runner
        .run(POWERSHELL, &["-NoProfile", "-Command", &script])
        .map(|_| ())
}

/// Post Control+`key`, the Linux/Windows analogue of Swift's Cmd+digit.
pub fn press_key(runner: &dyn Runner, os: Os, key: Key) -> Result<(), String> {
    match os {
        Os::Linux => runner
            .run(
                XDOTOOL,
                &[
                    "key",
                    "--clearmodifiers",
                    &format!("ctrl+{}", xdotool_keysym(key)),
                ],
            )
            .map(|_| ()),
        Os::Windows => sendkeys(runner, &format!("^{}", sendkeys_token(key))),
    }
}

/// Type literal text. On Linux `--` keeps a leading dash from being read as an
/// xdotool option; on Windows every SendKeys metacharacter is braced first.
pub fn type_text(runner: &dyn Runner, os: Os, text: &str) -> Result<(), String> {
    match os {
        Os::Linux => runner
            .run(XDOTOOL, &["type", "--clearmodifiers", "--", text])
            .map(|_| ()),
        Os::Windows => sendkeys(runner, &escape_sendkeys(text)),
    }
}

/// xdotool injects through XTEST, which an X11 (or XWayland) display provides.
/// Without the tool there is no honest way to claim a press, and without a
/// display the same tool cannot reach an application.
pub fn linux_capability(has_xdotool: bool, is_x11: bool) -> Capability {
    if !has_xdotool {
        return Capability::Unsupported("needs xdotool on PATH to post a key press".into());
    }
    if is_x11 {
        Capability::Supported
    } else {
        Capability::Degraded(
            "xdotool needs X11 or XWayland; a Wayland-only session cannot take a synthetic key"
                .into(),
        )
    }
}

/// PowerShell and `System.Windows.Forms.SendKeys` ship with Windows, so the
/// mechanism is present without a tool to probe for.
pub fn windows_capability() -> Capability {
    Capability::Supported
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    #[derive(Default)]
    struct RecordingRunner {
        calls: RefCell<Vec<(String, Vec<String>)>>,
    }

    impl Runner for RecordingRunner {
        fn run(&self, program: &str, args: &[&str]) -> Result<String, String> {
            self.calls.borrow_mut().push((
                program.to_owned(),
                args.iter().map(|arg| (*arg).to_owned()).collect(),
            ));
            Ok(String::new())
        }
    }

    struct FailingRunner;

    impl Runner for FailingRunner {
        fn run(&self, _program: &str, _args: &[&str]) -> Result<String, String> {
            Err("xdotool: command not found".into())
        }
    }

    #[test]
    fn only_warp_tab_digits_are_accepted() {
        for name in ["1", "2", "3", "4", "5", "6", "7", "8", "9"] {
            assert!(Key::parse(name).is_ok(), "{name} is a key Swift posts");
        }
        for name in ["0", "10", "a", "ctrl+1", "1; rm -rf /", ""] {
            let error = Key::parse(name).expect_err("not a key Swift posts");
            assert!(error.contains("unknown key"), "{error}");
        }
    }

    #[test]
    fn keysyms_and_sendkeys_tokens_follow_the_digit() {
        for digit in 1..=9 {
            let key = Key::parse(&digit.to_string()).unwrap();
            assert_eq!(xdotool_keysym(key), digit.to_string());
            assert_eq!(sendkeys_token(key), digit.to_string());
        }
    }

    #[test]
    fn sendkeys_specials_become_literals() {
        assert_eq!(
            escape_sendkeys("a{b}c+d^e%f~g(h)i"),
            "a{{}b{}}c{+}d{^}e{%}f{~}g{(}h{)}i"
        );
        assert_eq!(escape_sendkeys("plain text"), "plain text");
    }

    #[test]
    fn linux_press_uses_an_xdotool_argument_array() {
        let runner = RecordingRunner::default();
        press_key(&runner, Os::Linux, Key::parse("7").unwrap()).unwrap();
        assert_eq!(
            runner.calls.borrow()[0],
            (
                XDOTOOL.to_owned(),
                vec!["key", "--clearmodifiers", "ctrl+7"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect()
            )
        );
    }

    #[test]
    fn windows_press_adds_the_control_modifier() {
        let runner = RecordingRunner::default();
        press_key(&runner, Os::Windows, Key::parse("1").unwrap()).unwrap();
        let call = runner.calls.borrow();
        let (program, args) = &call[0];
        assert_eq!(program, POWERSHELL);
        assert_eq!(args[0], "-NoProfile");
        assert_eq!(args[1], "-Command");
        assert!(args[2].contains("SendWait('^1')"), "{}", args[2]);
    }

    #[test]
    fn linux_type_keeps_text_out_of_the_option_list() {
        let runner = RecordingRunner::default();
        type_text(&runner, Os::Linux, "--not-an-option; $(id)").unwrap();
        assert_eq!(
            runner.calls.borrow()[0],
            (
                XDOTOOL.to_owned(),
                vec!["type", "--clearmodifiers", "--", "--not-an-option; $(id)"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect()
            )
        );
    }

    #[test]
    fn windows_type_escapes_text_before_it_reaches_powershell() {
        let runner = RecordingRunner::default();
        type_text(&runner, Os::Windows, "it's ^good; {yes}").unwrap();
        let call = runner.calls.borrow();
        let (program, args) = &call[0];
        assert_eq!(program, POWERSHELL);
        assert!(
            args[2].contains("SendWait('it''s {^}good; {{}yes{}}')"),
            "{}",
            args[2]
        );
    }

    #[test]
    fn a_failed_command_is_returned_not_panicked_on() {
        let error = press_key(&FailingRunner, Os::Linux, Key::parse("1").unwrap())
            .expect_err("the runner failed");
        assert!(error.contains("not found"));
    }

    #[test]
    fn capability_follows_the_tool_and_the_display() {
        assert_eq!(linux_capability(true, true), Capability::Supported);
        let missing = linux_capability(false, true);
        assert!(matches!(missing, Capability::Unsupported(_)));
        assert!(missing.reason().unwrap().contains("xdotool"));
        let limited = linux_capability(true, false);
        assert!(matches!(limited, Capability::Degraded(_)));
        assert!(limited.reason().unwrap().contains("XWayland"));
        assert_eq!(windows_capability(), Capability::Supported);
    }
}
