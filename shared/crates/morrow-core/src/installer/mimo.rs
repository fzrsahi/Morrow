//! MiMo Code and AiWork (#234 batch 5): the three sources that share someone
//! else's plugin.
//!
//! MiMo Code forked OpenCode and kept its plugin format, so the plugin is **the
//! same file** re-labelled. macOS rewrites one line:
//!
//! ```text
//! const SOURCE = "opencode";   →   const SOURCE = "mimo";
//! ```
//!
//! The rewrite is deliberately exact — the marker must appear **once**, and an
//! artefact that no longer contains it yields `None` rather than a plugin that loads
//! and reports the wrong source. That is the failure this module exists to prevent:
//! a silently mislabelled plugin puts MiMo's events on OpenCode's cards.
//!
//! AiWork is different in kind, and the difference is worth stating: it has **no
//! hooks to install at all**. macOS treats "activated" as "monitoring is enabled and
//! the app is present", so there is no file to write and no uninstall to perform. A
//! row, and nothing else.

/// The line the OpenCode plugin spells its source on.
pub const OPENCODE_SOURCE_MARKER: &str = "const SOURCE = \"opencode\";";

/// The line MiMo's copy spells instead.
pub const MIMO_SOURCE_LINE: &str = "const SOURCE = \"mimo\";";

/// The OpenCode plugin, re-labelled for MiMo Code.
///
/// `None` when the marker is absent or present more than once: a plugin we cannot
/// re-label with certainty is one we do not ship.
pub fn mimo_plugin_source(opencode_source: &str) -> Option<String> {
    if opencode_source.matches(OPENCODE_SOURCE_MARKER).count() != 1 {
        return None;
    }
    Some(opencode_source.replace(OPENCODE_SOURCE_MARKER, MIMO_SOURCE_LINE))
}

/// Whether this plugin file is one we wrote.
///
/// The re-labelled line is the proof: nothing else on the machine writes
/// `const SOURCE = "mimo";` into a plugin called `morrow.js`. Requiring the app's
/// name as well would be checking for a string the ported artefact does not
/// necessarily contain.
pub fn is_our_mimo_plugin(text: &str) -> bool {
    text.contains(MIMO_SOURCE_LINE)
}

/// Where MiMo keeps plugins, relative to the home.
pub const MIMO_PLUGIN_PATH: &str = ".config/mimocode/plugins/morrow.js";

/// Where the `mimo` CLI installs itself: a second sign that MiMo is on the machine.
pub const MIMO_INSTALL_DIR: &str = ".mimocode";

/// AiWork's state directory: the TUI brings up an Agentix daemon under
/// `~/.agentix` even when the GUI is not installed, which is the check that decides
/// in practice.
pub const AIWORK_STATE_DIR: &str = ".agentix";

#[cfg(test)]
mod tests {
    use super::*;

    const PLUGIN: &str = "const SOURCE = \"opencode\";\nconst CODEISLAND = 1;\n";

    #[test]
    fn the_plugin_is_relabelled_by_one_exact_line() {
        let mimo = mimo_plugin_source(PLUGIN).expect("a re-labelled plugin");
        assert!(mimo.contains(MIMO_SOURCE_LINE));
        assert!(!mimo.contains(OPENCODE_SOURCE_MARKER));
        // Everything else is untouched, byte for byte.
        assert!(mimo.contains("const CODEISLAND = 1;"));
        assert_eq!(
            mimo.len(),
            PLUGIN.len() - OPENCODE_SOURCE_MARKER.len() + MIMO_SOURCE_LINE.len()
        );
    }

    #[test]
    fn an_artefact_that_lost_its_marker_is_refused_rather_than_shipped() {
        // A plugin that loads and reports OpenCode would put MiMo's events on
        // OpenCode's cards, and nothing would look broken.
        assert!(mimo_plugin_source("// nothing to relabel\n").is_none());
        // Twice is ambiguous: which occurrence is the source line?
        assert!(mimo_plugin_source(&format!("{PLUGIN}{PLUGIN}")).is_none());
        assert!(mimo_plugin_source("").is_none());
    }

    #[test]
    fn only_a_plugin_that_names_us_and_mimo_is_ours() {
        let mimo = mimo_plugin_source(PLUGIN).expect("a plugin");
        assert!(is_our_mimo_plugin(&mimo));
        // OpenCode's own copy is not MiMo's, even though it is ours.
        assert!(!is_our_mimo_plugin(PLUGIN));
        assert!(is_our_mimo_plugin("const SOURCE = \"mimo\";"));
    }
}
