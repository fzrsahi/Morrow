//! Cline's hooks (#234 batch 4): a directory of scripts, not a JSON key.
//!
//! Cline has no config file. It looks for an executable named after the event in
//! `~/Documents/Cline/Hooks/`, so an install is eight files and an uninstall is
//! eight files — there is nothing to merge, and nothing of the user's can be
//! clobbered as long as we only ever write the eight names we own.
//!
//! Every script must print `{"cancel":false}` and exit **at once**: Cline blocks on
//! the hook, and a bridge that talked to the island first would hold the tool call
//! for as long as nobody answered. So the script backgrounds the bridge and answers
//! immediately.

/// What every script of ours carries, and the only proof that a file is ours.
///
/// Not `codeisland-bridge`: that is the macOS binary's name, and the command this
/// port writes names the hook script instead. Keying the marker on a string the
/// script does not actually contain is how uninstall would leave eight files
/// behind while the card said "absent".
pub const MARKER: &str = "--source cline";

/// The eight hooks Cline looks for, in its own order.
///
/// `TaskStart`/`TaskResume`/`TaskCancel`/`TaskComplete` exist only here: Cline
/// models work as tasks, not as turns.
pub const EVENTS: &[&str] = &[
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "TaskStart",
    "TaskResume",
    "TaskCancel",
    "TaskComplete",
    "PreCompact",
];

/// The hook script, for the command this platform installs.
///
/// `command` is the same string every other target names — the hook script with
/// `--source cline` — so there is one description of where the bridge is. The
/// original hard-coded `~/.codeisland/codeisland-bridge`, which is right on macOS
/// and wrong here: the bridge is `morrow-bridge`.
pub fn script(command: &str) -> String {
    [
        "#!/bin/bash".to_owned(),
        "INPUT=$(cat)".to_owned(),
        format!("printf '%s' \"$INPUT\" | {command} \"$@\" >/dev/null 2>&1 &"),
        // Cline needs valid JSON on stdout from every invocation, immediately.
        "printf '{\"cancel\":false}'".to_owned(),
        String::new(),
    ]
    .join("\n")
}

/// Whether this script is one we wrote.
pub fn is_ours(text: &str) -> bool {
    text.contains(MARKER) && text.contains("codeisland")
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMAND: &str = "~/.codeisland/codeisland-hook.sh --source cline";

    #[test]
    fn the_script_backgrounds_the_bridge_and_answers_at_once() {
        let script = script(COMMAND);
        // The answer is printed on its own line, after the backgrounded bridge —
        // Cline reads it immediately.
        assert!(script.contains(r#"printf '{"cancel":false}'"#));
        assert!(script.contains("&"), "the bridge must not block the answer");
        assert!(script.contains("--source cline"));
        assert!(script.starts_with("#!/bin/bash\n"));
        // And it names the command it was given, not a hard-coded macOS path.
        assert!(!script.contains("codeisland-bridge"), "{script}");
        // The marker really is in the script: a marker that is only in a comment
        // is a marker that quietly stops matching.
        assert!(is_ours(&script));
    }

    #[test]
    fn every_event_of_clines_own_list_is_written() {
        // Eight, and four of them are task events no other target has.
        assert_eq!(EVENTS.len(), 8);
        for event in ["TaskStart", "TaskResume", "TaskCancel", "TaskComplete"] {
            assert!(EVENTS.contains(&event), "{event} is Cline's own");
        }
    }

    #[test]
    fn a_script_is_only_ours_when_it_says_so() {
        assert!(is_ours(&script(COMMAND)));
        // The user's own hook for the same event is not ours to delete.
        assert!(!is_ours("#!/bin/bash\necho mine\n"));
    }
}
