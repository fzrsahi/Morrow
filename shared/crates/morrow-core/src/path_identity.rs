//! What a config path *is* on disk, as opposed to how it was spelled (#247b).
//!
//! Multi-account setups reach one directory or file through several spellings:
//! `~/.claude-work` and a symlink to it, a hand-typed `~/.Codex-Work` on a
//! case-insensitive volume, a `settings.json` symlinked between two accounts.
//! Comparing spellings then gets "is this the primary's file?" wrong, which is
//! how removing an extra directory could strip the primary's hooks — and how
//! one account's sessions get scanned twice and counted as two cards (#248).
//!
//! ## Deliberately not ported: `writeTarget`
//!
//! `ConfigPathIdentity.writeTarget` exists because Swift's `String.write(atomically:)`
//! replaces a file by renaming a temporary copy over it, which turns a symlinked
//! `settings.json` (dotfiles, one file shared between accounts) into a detached
//! regular file. Rust's `fs::write` opens the path and truncates *through* the
//! link, so the link survives — the bug has no port to fix. There is a test
//! below that pins that property, because the obvious "make the write atomic"
//! improvement would silently reintroduce exactly the breakage Swift had to work
//! around.

use std::path::Path;

use crate::config_roots::PathStyle;

/// Symlink hops before a cycle is assumed (the kernel's own `MAXSYMLINKS`).
const MAX_HOPS: u32 = 32;

/// `ConfigPathIdentity.resolved`: every symlink followed, every existing
/// component in its on-disk spelling.
///
/// A path that does not exist (yet) keeps its missing tail as written below its
/// deepest existing ancestor, and a dangling symlink is followed to where it
/// points — so `extra/hooks/codeisland.json` inside a symlinked `hooks/` resolves
/// to the linked folder before the file is ever written.
///
/// Host-accurate on purpose: this asks the filesystem, so it uses `std::path`
/// rather than an injected style. Only [`lexically_normalized`], which never
/// touches disk, takes one.
pub fn resolved(path: &str) -> String {
    resolved_hops(path, 0)
}

/// Key for deciding that two spellings are the same file or directory.
///
/// Windows folds case: its filesystems are case-insensitive, and a path that
/// does not exist yet cannot be canonicalized, so `C:\Users\Dev\.claude` and
/// `c:\users\dev\.claude` would otherwise be two roots. The result is a
/// comparison key, never displayed. Elsewhere the answer is the canonical form
/// itself — on macOS `realpath(3)` already restores the stored case on a
/// case-insensitive volume, which is the normalization Swift got from NFC.
pub fn identity(path: &str) -> String {
    let resolved = resolved(path);
    if cfg!(windows) {
        resolved.to_lowercase()
    } else {
        resolved
    }
}

fn resolved_hops(path: &str, hops: u32) -> String {
    if let Ok(real) = std::fs::canonicalize(path) {
        return strip_verbatim(&real.to_string_lossy());
    }
    // A relative path has no meaning here: the caller's cwd is not ours.
    if hops >= MAX_HOPS || !Path::new(path).is_absolute() {
        return path.to_owned();
    }

    // A dangling symlink: `canonicalize` gives up, but a write would land at its
    // target, so that is what the path is.
    if let Ok(destination) = std::fs::read_link(path) {
        let parent = Path::new(path)
            .parent()
            .map(|parent| parent.to_string_lossy().into_owned())
            .unwrap_or_default();
        let destination = destination.to_string_lossy();
        let target = if Path::new(destination.as_ref()).is_absolute() {
            destination.into_owned()
        } else {
            join(&parent, &destination)
        };
        let target = lexically_normalized(&target, PathStyle::current());
        return resolved_hops(&target, hops + 1);
    }

    // Nothing there (yet): resolve the deepest existing ancestor and keep the
    // missing tail as written.
    let path_ref = Path::new(path);
    let (Some(parent), Some(name)) = (path_ref.parent(), path_ref.file_name()) else {
        return path.to_owned();
    };
    let parent = parent.to_string_lossy();
    if parent.is_empty() || parent == path {
        return path.to_owned();
    }
    let base = resolved_hops(&parent, hops);
    join(&base, &name.to_string_lossy())
}

/// `.` and `..` collapsed textually, for a relative symlink target joined onto
/// its link's folder. Only used on paths `canonicalize` could not take.
///
/// Takes a style because it is pure: a Windows-shaped path can be normalized on
/// a POSIX host, which is where the rules actually get tested.
pub fn lexically_normalized(path: &str, style: PathStyle) -> String {
    let (prefix, rest) = split_prefix(path, style);
    let mut parts: Vec<&str> = Vec::new();
    for component in rest.split(style.separator()) {
        if component.is_empty() || component == "." {
            continue;
        }
        if component == ".." {
            parts.pop();
            continue;
        }
        parts.push(component);
    }
    let joined = parts.join(&style.separator().to_string());
    match (prefix, joined.is_empty()) {
        (Some(prefix), true) => prefix,
        (Some(prefix), false) => format!("{prefix}{joined}"),
        (None, true) => String::new(),
        (None, false) => joined,
    }
}

/// The part of `path` a lexical walk must not treat as components: `C:\`,
/// `\\server\share`, or the leading separator of a rooted POSIX path.
fn split_prefix(path: &str, style: PathStyle) -> (Option<String>, &str) {
    match style {
        // A rooted path keeps its separator: it is not a component, and
        // dropping it turns `/a/c` into the relative `a/c`.
        PathStyle::Posix => match path.strip_prefix('/') {
            Some(rest) => (Some("/".to_owned()), rest),
            None => (None, path),
        },
        PathStyle::Windows => {
            let bytes = path.as_bytes();
            if bytes.len() >= 3 && bytes[1] == b':' && style.is_separator(bytes[2] as char) {
                return (Some(path[..3].to_owned()), &path[3..]);
            }
            if path.starts_with("\\\\") || path.starts_with("//") {
                // \\server\share — the share is part of the root, not a
                // component that `..` may pop.
                let mut seps = 0;
                for (index, c) in path.char_indices().skip(2) {
                    if style.is_separator(c) {
                        seps += 1;
                        if seps == 2 {
                            return (Some(path[..=index].to_owned()), &path[index + 1..]);
                        }
                    }
                }
                return (Some(path.to_owned()), "");
            }
            (None, path)
        }
    }
}

fn join(base: &str, name: &str) -> String {
    PathStyle::current().join(base, name)
}

/// `std::fs::canonicalize` on Windows answers an extended-length path. Its
/// `\\?\` prefix is not part of the path a user typed, and `\\?\UNC\server\share`
/// is spelled `\\server\share`, so both are put back before the value reaches a
/// comparison or the UI.
fn strip_verbatim(path: &str) -> String {
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    path.strip_prefix(r"\\?\").unwrap_or(path).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn style() -> PathStyle {
        PathStyle::Posix
    }

    /// A scratch directory that removes itself, so a failing assertion cannot
    /// leave symlinks behind in `/tmp`.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("morrow-identity-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("a scratch directory");
            Self(path)
        }

        fn join(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }

        fn text(&self, name: &str) -> String {
            self.join(name).to_string_lossy().into_owned()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_lexical_walk_collapses_dots_without_escaping_the_root() {
        assert_eq!(lexically_normalized("/a/b/../c/./d", style()), "/a/c/d");
        // `..` at the root has nowhere to go.
        assert_eq!(lexically_normalized("/../a", style()), "/a");
        assert_eq!(lexically_normalized("/a/../..", style()), "/");
        assert_eq!(lexically_normalized("/", style()), "/");
    }

    #[test]
    fn a_lexical_walk_keeps_a_windows_root() {
        let windows = PathStyle::Windows;
        assert_eq!(
            lexically_normalized(r"C:\Users\dev\..\other\.\file", windows),
            r"C:\Users\other\file"
        );
        // The drive is not a component, so it cannot be popped.
        assert_eq!(lexically_normalized(r"C:\..\a", windows), r"C:\a");
        // A UNC share is part of the root for the same reason.
        assert_eq!(
            lexically_normalized(r"\\nas\share\..\a", windows),
            r"\\nas\share\a"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_and_its_target_are_one_identity() {
        let scratch = Scratch::new("symlink");
        fs::create_dir_all(scratch.join("real")).expect("a directory");
        std::os::unix::fs::symlink(scratch.join("real"), scratch.join("link")).expect("a symlink");

        assert_eq!(
            identity(&scratch.text("link")),
            identity(&scratch.text("real")),
            "a symlinked root must not be scanned as a second root"
        );
        // And through the link, so a file below it is the same file.
        assert_eq!(
            identity(&format!("{}/child.json", scratch.text("link"))),
            identity(&format!("{}/child.json", scratch.text("real")))
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_dangling_symlink_resolves_to_where_a_write_would_land() {
        let scratch = Scratch::new("dangling");
        std::os::unix::fs::symlink(scratch.join("missing"), scratch.join("link"))
            .expect("a symlink");

        let through_link = resolved(&scratch.text("link"));
        assert!(
            through_link.ends_with("missing"),
            "a link to a file that does not exist yet still names that file: {through_link}"
        );
        assert_eq!(
            through_link,
            resolved(&scratch.text("missing")),
            "the dangling link and its target are the same path"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_loop_terminates() {
        let scratch = Scratch::new("loop");
        std::os::unix::fs::symlink(scratch.join("b"), scratch.join("a")).expect("a symlink");
        std::os::unix::fs::symlink(scratch.join("a"), scratch.join("b")).expect("a symlink");

        // The assertion is that this returns at all.
        let resolved = resolved(&scratch.text("a"));
        assert!(resolved.contains("morrow-identity"));
    }

    #[test]
    fn a_missing_tail_is_kept_below_its_deepest_existing_ancestor() {
        let scratch = Scratch::new("missing-tail");
        let wanted = scratch.text("hooks/codeisland.json");

        // Nothing exists below the scratch dir, so the tail is as written.
        // `resolved` still canonicalizes the existing ancestor — on macOS
        // `/var` is a symlink to `/private/var`, so the prefix legitimately
        // differs while the tail does not.
        assert!(resolved(&wanted).ends_with("hooks/codeisland.json"));
        let before = resolved(&wanted);
        fs::create_dir_all(scratch.join("hooks")).expect("a directory");
        assert_eq!(
            resolved(&wanted),
            before,
            "creating the parent must not change the answer"
        );
    }

    #[test]
    fn a_relative_path_is_left_alone() {
        // Resolving it against *our* cwd would be worse than useless.
        assert_eq!(resolved("relative/path"), "relative/path");
        assert_eq!(identity("relative/path"), "relative/path");
    }

    /// The property that makes `writeTarget` unnecessary here. If a future
    /// change makes this write atomic (write a temp file, rename it over), a
    /// dotfiles-managed `settings.json` stops being a symlink and the user's
    /// other account silently stops seeing our hooks.
    #[cfg(unix)]
    #[test]
    fn writing_through_a_symlink_keeps_the_link() {
        let scratch = Scratch::new("write");
        fs::write(scratch.join("real.json"), b"{\"before\":true}").expect("a file");
        std::os::unix::fs::symlink(scratch.join("real.json"), scratch.join("link.json"))
            .expect("a symlink");

        fs::write(scratch.join("link.json"), b"{\"after\":true}").expect("a write");

        assert!(
            fs::symlink_metadata(scratch.join("link.json"))
                .expect("metadata")
                .is_symlink(),
            "the link must survive the write"
        );
        assert_eq!(
            fs::read_to_string(scratch.join("real.json")).expect("the target"),
            "{\"after\":true}"
        );
    }

    /// A symlinked directory on the way is followed by the kernel, so only the
    /// file's own link matters — the same conclusion Swift reached.
    #[cfg(unix)]
    #[test]
    fn writing_below_a_symlinked_directory_reaches_the_real_folder() {
        let scratch = Scratch::new("write-dir");
        fs::create_dir_all(scratch.join("real")).expect("a directory");
        std::os::unix::fs::symlink(scratch.join("real"), scratch.join("link")).expect("a symlink");

        fs::write(scratch.join("link/settings.json"), b"{}").expect("a write");

        assert_eq!(
            fs::read_to_string(scratch.join("real/settings.json")).expect("the real file"),
            "{}"
        );
    }
}
