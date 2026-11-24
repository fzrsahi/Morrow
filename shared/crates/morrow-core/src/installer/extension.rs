//! The TypeScript extensions (#234 batch 4b): pi, Oh My Pi and OpenClaw.
//!
//! These three are not config files. Each is a whole extension the CLI loads from a
//! directory it scans, and the file is the artefact — 710, 1,118 and 578 lines of
//! TypeScript that ship with the app.
//!
//! The port cannot ship them unchanged, and this is the reason this module exists:
//! every one of them finds the bridge by a hard-coded macOS path.
//!
//! ```text
//! const BRIDGE_PATH = `${homedir()}/.codeisland/codeisland-bridge`;
//! ```
//!
//! On Linux and Windows the bridge is `morrow-bridge`, so an unchanged extension
//! would look for a binary that does not exist and report nothing — a plugin that
//! loads, does nothing, and looks installed.
//!
//! So the install rewrites that one line, and **refuses** when it cannot find it
//! exactly once. An artefact that changed shape is an artefact we do not understand,
//! and guessing would put a silent no-op on the user's machine.

/// The line each bundled extension uses to find the bridge.
pub const BRIDGE_MARKER: &str = "const BRIDGE_PATH = `${homedir()}/.codeisland/codeisland-bridge`;";

/// The extension text with the bridge path pointed at this platform's binary.
///
/// `None` when the marker is absent or appears more than once: a marker we cannot
/// locate with certainty is one we do not rewrite.
pub fn with_bridge_path(source: &str, bridge: &str) -> Option<String> {
    if source.matches(BRIDGE_MARKER).count() != 1 {
        return None;
    }
    // A plain string literal, not a template: the path is absolute by the time it
    // reaches here, so there is nothing left to interpolate.
    let escaped = bridge.replace('\\', "\\\\").replace('"', "\\\"");
    Some(source.replace(
        BRIDGE_MARKER,
        &format!("const BRIDGE_PATH = \"{escaped}\";"),
    ))
}

/// Whether this extension is one we wrote.
///
/// The rewritten line is the proof: the bundled artefact uses a template literal,
/// so a double-quoted `BRIDGE_PATH` only occurs after an install.
pub fn is_ours(text: &str) -> bool {
    text.contains("const BRIDGE_PATH = \"")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artefact() -> String {
        format!("import {{ existsSync }} from \"node:fs\";\n{BRIDGE_MARKER}\nconst OTHER = 1;\n")
    }

    #[test]
    fn the_bridge_line_becomes_an_absolute_path() {
        let rewritten = with_bridge_path(&artefact(), "/usr/lib/morrow/morrow-bridge")
            .expect("a rewritten extension");
        assert!(rewritten.contains("const BRIDGE_PATH = \"/usr/lib/morrow/morrow-bridge\";"));
        assert!(!rewritten.contains("codeisland-bridge"));
        // Everything else is untouched.
        assert!(rewritten.contains("const OTHER = 1;"));
        assert!(rewritten.contains("import { existsSync }"));
    }

    #[test]
    fn a_windows_path_is_escaped_for_a_typescript_string() {
        let rewritten = with_bridge_path(&artefact(), r"C:\Users\dev\morrow-bridge.exe")
            .expect("a rewritten extension");
        assert!(
            rewritten.contains(r#"const BRIDGE_PATH = "C:\\Users\\dev\\morrow-bridge.exe";"#),
            "{rewritten}"
        );
    }

    #[test]
    fn an_artefact_that_lost_its_marker_is_refused_rather_than_shipped() {
        // Shipping it unchanged is a plugin that looks for the macOS binary and
        // silently reports nothing.
        assert!(with_bridge_path("// nothing here\n", "/x").is_none());
        assert!(with_bridge_path(&artefact().repeat(2), "/x").is_none());
        assert!(with_bridge_path("", "/x").is_none());
    }

    #[test]
    fn only_a_rewritten_extension_is_ours() {
        let bundled = artefact();
        assert!(!is_ours(&bundled), "the bundled artefact is not an install");
        assert!(is_ours(
            &with_bridge_path(&bundled, "/x").expect("a rewrite")
        ));
    }
}
