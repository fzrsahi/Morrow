//! #239 AC 1 — a registered IPC command nobody calls is dead surface.
//!
//! `generate_handler![…]` in `main.rs` is a promise: each name is reachable from
//! the webview over Tauri's IPC bridge. Seven names in that list had no caller
//! when this was measured, and the ticket was wrong about one of them
//! (`music_position` was described as used; it was called from nowhere at all).
//! Reading the list by hand found that once, three PRs late, so the check is a
//! test rather than a habit.
//!
//! Two directions, and both matter:
//!
//! - a **registered** command must be named by the frontend, or appear in
//!   [`INTERNAL_COMMANDS`] with a reason;
//! - an entry in [`INTERNAL_COMMANDS`] must still be internal — a stale exception
//!   quietly authorises the next orphan.
//!
//! Rust-internal helpers whose *function* is used by the shell (`process_watch`,
//! `process_alive`, `notify_send`) are not IPC commands and are not registered, so
//! they do not appear here at all: the check is about the handler list only.

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;

    /// Commands that are registered on purpose without a frontend caller. Empty,
    /// and a test below fails if one is added without the other side changing:
    /// every command this shell registers is one the webview actually asks for.
    const INTERNAL_COMMANDS: &[&str] = &[];

    fn crate_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    /// The names inside `generate_handler![…]`, last segment only
    /// (`island::reset_island_position` → `reset_island_position`).
    fn registered(main_rs: &str) -> BTreeSet<String> {
        let start = main_rs
            .find("generate_handler![")
            .expect("main.rs has a handler list");
        let rest = &main_rs[start + "generate_handler![".len()..];
        let end = rest.find("])").expect("the handler list is closed");
        rest[..end]
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty() && !name.starts_with("//"))
            .map(|name| name.rsplit("::").next().unwrap_or(name).to_owned())
            .collect()
    }

    fn frontend_source() -> String {
        let mut out = String::new();
        let mut stack = vec![crate_root().join("../src")];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if matches!(
                    path.extension().and_then(|ext| ext.to_str()),
                    Some("svelte" | "ts" | "js" | "html")
                ) {
                    out.push_str(&fs::read_to_string(&path).unwrap_or_default());
                    out.push('\n');
                }
            }
        }
        out
    }

    fn quoted(source: &str, name: &str) -> bool {
        source.contains(&format!("'{name}'")) || source.contains(&format!("\"{name}\""))
    }

    /// Names referenced from Rust outside `main.rs`, ignoring the line that
    /// *defines* the function — a definition is not a caller.
    fn called_from_rust(name: &str) -> usize {
        let mut count = 0;
        let mut stack = vec![crate_root().join("src")];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                    continue;
                }
                // `main.rs` registers names; this file names them to audit them.
                // Neither is a caller — counting them is how the first audit
                // reported seven orphans as "used by the module that defines
                // them" and found nothing.
                if matches!(
                    path.file_name().and_then(|name| name.to_str()),
                    Some("main.rs" | "commands.rs")
                ) {
                    continue;
                }
                let Ok(text) = fs::read_to_string(&path) else {
                    continue;
                };
                for line in text.lines() {
                    let defines = line.contains(&format!("fn {name}("))
                        || line.contains(&format!("fn {name}<"));
                    if defines {
                        continue;
                    }
                    if line.contains(name) {
                        count += 1;
                    }
                }
            }
        }
        count
    }

    fn main_rs() -> String {
        fs::read_to_string(crate_root().join("src/main.rs")).expect("main.rs is readable")
    }

    #[test]
    fn every_registered_command_is_asked_for_by_the_frontend() {
        let main_rs = main_rs();
        let frontend = frontend_source();
        assert!(frontend.len() > 1_000, "the frontend source did not load");

        let mut orphans = Vec::new();
        for name in registered(&main_rs) {
            if quoted(&frontend, &name) {
                continue;
            }
            if INTERNAL_COMMANDS.contains(&name.as_str()) {
                continue;
            }
            orphans.push(format!(
                "{name} — registered but never invoked (frontend 0 hits, Rust {})",
                called_from_rust(&name)
            ));
        }
        assert!(
            orphans.is_empty(),
            "registered commands with no caller:\n  {}",
            orphans.join("\n  ")
        );
    }

    #[test]
    fn an_internal_exception_is_still_internal() {
        // A stale exception authorises the next orphan by precedent.
        let frontend = frontend_source();
        for name in INTERNAL_COMMANDS {
            assert!(
                !quoted(&frontend, name),
                "{name} is listed as internal, but the frontend calls it"
            );
        }
    }

    #[test]
    fn the_three_helpers_the_shell_calls_itself_are_not_ipc_commands() {
        // #239 AC 1: these were registered with no frontend caller. They stay as
        // Rust functions because the shell calls them, and they must not be
        // registered again — the handler list is the webview's surface, not a
        // second name for every helper.
        let main_rs = main_rs();
        let registered = registered(&main_rs);
        for name in ["process_watch", "process_alive", "notify_send"] {
            assert!(!registered.contains(name), "{name} is IPC surface again");
            assert!(
                called_from_rust(name) > 0,
                "{name} is neither registered nor called — delete it"
            );
        }
    }

    #[test]
    fn the_parser_reads_the_handler_list_rather_than_the_whole_file() {
        // A guard that silently matched nothing would pass forever. The real list
        // is over fifty names; a parser that returned a handful would not be
        // reading `generate_handler!`.
        let registered = registered(&main_rs());
        assert!(
            registered.len() > 40,
            "only {} names found",
            registered.len()
        );
        assert!(registered.contains("settings_set"));
        assert!(
            !registered.contains("generate_handler"),
            "the parser is trimming the macro name, not a name list"
        );
    }
}
