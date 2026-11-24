//! `config_roots::roots` composed with the real `path_identity::identity`
//! (#247b).
//!
//! The two halves were built in separate slices and each was tested with an
//! injected fake: `roots` with a `to_lowercase`, `identity` against a scratch
//! tree. Neither proved that they agree, and disagreement is the bug that
//! matters — `ExtraConfigDirs`' own comment names it: "removing an extra
//! directory could strip the primary's hooks".
//!
//! No production caller yet: dedupe across roots lands with the discovery
//! coordinator (#248). This file is the seam being exercised before it is wired,
//! so the wiring cannot be the first time the two are combined.

use std::fs;
use std::path::PathBuf;

use morrow_core::config_roots::roots;
use morrow_core::path_identity::identity;

/// A scratch directory that removes itself, so a failing assertion cannot leave
/// a symlinked tree in `/tmp`.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "morrow-roots-identity-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("a scratch directory");
        Self(path)
    }

    fn text(&self, name: &str) -> String {
        self.0.join(name).to_string_lossy().into_owned()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The primary root and a symlink to it are **one** root. Without this, both
/// spellings get scanned and hooked, and a hook installed twice in the same file
/// is a hook that is removed once and still there.
#[cfg(unix)]
#[test]
fn a_symlink_to_the_primary_root_is_not_a_second_root() {
    let scratch = Scratch::new("primary");
    fs::create_dir_all(scratch.0.join("claude")).expect("a directory");
    std::os::unix::fs::symlink(scratch.0.join("claude"), scratch.0.join("alias"))
        .expect("a symlink");

    let primary = scratch.text("claude");
    let alias = scratch.text("alias");
    let other = scratch.text("work");

    let deduped = roots(&primary, &[&alias, &other], identity);

    assert_eq!(deduped, vec![primary.clone(), other]);
    assert!(
        !deduped.contains(&alias),
        "the alias spelling must not survive as its own root"
    );
}

/// Two genuinely different accounts stay two roots: the dedupe must not be so
/// eager that it collapses a real second config directory.
#[cfg(unix)]
#[test]
fn two_real_directories_stay_two_roots() {
    let scratch = Scratch::new("distinct");
    fs::create_dir_all(scratch.0.join("work")).expect("a directory");
    fs::create_dir_all(scratch.0.join("personal")).expect("a directory");

    let primary = scratch.text("work");
    let personal = scratch.text("personal");

    assert_eq!(
        roots(&primary, &[&personal], identity),
        vec![primary, personal]
    );
}

/// A root that does not exist yet is still a root — the installer creates it.
/// Two spellings of it cannot be resolved, so they stay distinct rather than
/// being folded by a guess.
#[test]
fn a_root_that_does_not_exist_yet_is_kept() {
    let scratch = Scratch::new("missing");
    let primary = scratch.text("never-created");

    assert_eq!(roots(&primary, &[], identity), vec![primary]);
}

/// The same directory spelled two ways is one root on a case-insensitive
/// filesystem, which is every Windows volume. This host cannot prove the
/// filesystem part, but it can prove the key: `identity` folds on Windows.
#[test]
fn identity_folds_case_on_windows_only() {
    let path = r"C:\Users\Dev\.claude-work";
    let folded = identity(path);
    if cfg!(windows) {
        assert_eq!(folded, path.to_lowercase());
        assert_eq!(folded, identity(r"c:\users\dev\.CLAUDE-WORK"));
    } else {
        // On a POSIX host a Windows-shaped path is not absolute, so it is left
        // exactly as written — it must not be mistaken for a real path.
        assert_eq!(folded, path);
    }
}
