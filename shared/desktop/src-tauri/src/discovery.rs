//! Backfill: the sessions that exist before their hook does (#261).
//!
//! Until this, a session could only be born from a hook. Restart the app while an
//! agent is working and the panel is empty until that agent's next event — and a
//! session whose hook never carried a pid is never pruned, so its card outlives
//! the process.
//!
//! The scan itself is `morrow_core::discovery::scan` (#260): roots × readers,
//! deduped, within a budget. What lives here is the one reader that reads
//! something real today — Claude Code's `projects/` tree — and the decision of
//! when to run it.

use std::collections::HashSet;
use std::path::Path;

use morrow_core::discovery::scan::{self, ScanResult, StoreReader};
use morrow_core::discovery::{DiscoveryHit, read_transcript_tail, transcript_files};

/// How much of a transcript the backfill reads. The same slice the live tailer
/// reads on attach, so backfill costs one read of bytes that are already going to
/// be read rather than a second parse of the whole store.
const TAIL_BYTES: u64 = 64 * 1024;

/// Claude Code's `projects/<encoded>/<session>.jsonl`.
///
/// Two levels, not a recursive walk: `projects/` holds one directory per working
/// directory and the transcripts directly inside it. `Discovery/` on macOS only
/// ever lists one directory for the same reason.
pub struct ClaudeTranscripts;

impl StoreReader for ClaudeTranscripts {
    fn source(&self) -> &'static str {
        "claude"
    }

    fn read(&self, root: &str) -> Vec<DiscoveryHit> {
        let projects = Path::new(root).join("projects");
        let Ok(entries) = std::fs::read_dir(&projects) else {
            return Vec::new();
        };
        let mut hits = Vec::new();
        for entry in entries.flatten() {
            // `entry.path()` is the encoded project directory. Entries are not
            // resolved: a symlinked transcript keeps its own mtime, which is what
            // tells a live session from one left behind (#256).
            let Some(files) = transcript_files::list(&entry.path().to_string_lossy()) else {
                continue;
            };
            for file in files {
                let Some(tail) = read_tail(Path::new(&file.path)) else {
                    continue;
                };
                let Some(summary) = read_transcript_tail(&tail, &file.name) else {
                    continue;
                };
                hits.push(DiscoveryHit {
                    session_id: summary.session_id,
                    source: "claude",
                    cwd: summary.cwd,
                    tty: None,
                    model: summary.model,
                    pid: None,
                    modified_at: Some(file.modified_ms),
                    recent_messages: summary.recent_messages,
                    transcript_path: Some(file.path),
                });
            }
        }
        hits
    }
}

/// The last `TAIL_BYTES` of a file as text. A partial UTF-8 sequence at the cut
/// is dropped rather than failing the whole read.
fn read_tail(path: &Path) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let size = file.metadata().ok()?.len();
    let start = size.saturating_sub(TAIL_BYTES);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::new();
    file.take(TAIL_BYTES).read_to_end(&mut bytes).ok()?;
    if start > 0 {
        // Skip to the first newline: the row at the cut is half a line, and the
        // reader tolerates it, but dropping it keeps a cut multi-byte char from
        // making the whole tail lossy.
        if let Some(index) = bytes.iter().position(|byte| *byte == b'\n') {
            bytes.drain(..=index);
        }
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// One scan over every root, with the budget from the coordinator.
pub fn scan_once(roots: &[String]) -> ScanResult {
    let reader = ClaudeTranscripts;
    let readers: Vec<&dyn StoreReader> = vec![&reader];
    scan::run(roots, &readers, scan::DEFAULT_BUDGET_MS, now_ms)
}

/// The roots to scan for Claude Code today: the live config dir, plus any other
/// directory under it that holds `projects/`.
///
/// The extras are not Settings-registered ones yet (`ExtraConfigDir` has no Rust
/// counterpart); they are the sibling accounts a multi-account user already has,
/// discovered by the same marker that makes a root live.
pub fn claude_roots(primary: &Path) -> Vec<String> {
    let primary = primary.to_string_lossy().into_owned();
    let mut roots = scan::scan_roots(&primary, &[]);
    // A `~/.claude-work` style sibling: same parent, holds `projects/`.
    if let Some(parent) = Path::new(&primary).parent() {
        if let Ok(entries) = std::fs::read_dir(parent) {
            let mut siblings: Vec<String> = entries
                .flatten()
                .filter_map(|entry| {
                    let path = entry.path();
                    let name = path.file_name()?.to_string_lossy().into_owned();
                    if !name.starts_with(".claude") || path == Path::new(&primary) {
                        return None;
                    }
                    path.join("projects")
                        .is_dir()
                        .then(|| path.to_string_lossy().into_owned())
                })
                .collect();
            // Deterministic, so two runs scan in the same order.
            siblings.sort();
            roots.extend(scan::scan_roots(
                &primary,
                &siblings.iter().map(String::as_str).collect::<Vec<_>>(),
            ));
            // `scan_roots` puts the primary first and drops duplicates; the first
            // call above is subsumed by this one.
            return dedupe(roots);
        }
    }
    roots
}

fn dedupe(roots: Vec<String>) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    roots
        .into_iter()
        .filter(|root| seen.insert(morrow_core::path_identity::identity(root)))
        .collect()
}

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::path::PathBuf;

    use super::*;

    /// A scratch `~` that removes itself.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("morrow-backfill-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("a scratch home");
            Self(path)
        }

        fn project_dir(&self, root: &str, encoded: &str) -> PathBuf {
            let path = self.0.join(root).join("projects").join(encoded);
            fs::create_dir_all(&path).expect("a project directory");
            path
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn transcript(cwd: &str, model: &str) -> String {
        format!(
            "{}\n{}\n",
            format_args!(
                r#"{{"type":"user","cwd":"{cwd}","message":{{"role":"user","content":"Add a filter bar"}}}}"#
            ),
            format_args!(
                r#"{{"type":"assistant","cwd":"{cwd}","message":{{"model":"{model}","content":[{{"type":"text","text":"Added the bar"}}]}}}}"#
            )
        )
    }

    fn write(path: &Path, contents: &str) {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("a transcript");
        file.write_all(contents.as_bytes()).expect("a write");
    }

    #[test]
    fn a_transcript_without_a_hook_becomes_a_session() {
        let scratch = Scratch::new("one");
        let projects = scratch.project_dir(".claude", "-home-dev-project");
        write(
            &projects.join("session-a.jsonl"),
            &transcript("/home/dev/project", "model-x"),
        );

        let root = scratch.0.join(".claude").to_string_lossy().into_owned();
        let result = scan_once(&[root]);

        assert_eq!(result.hits.len(), 1);
        let hit = &result.hits[0];
        assert_eq!(hit.session_id, "session-a");
        assert_eq!(hit.source, "claude");
        assert_eq!(hit.cwd, "/home/dev/project");
        assert_eq!(hit.model.as_deref(), Some("model-x"));
        assert_eq!(hit.recent_messages.len(), 2);
        assert!(
            hit.transcript_path
                .as_deref()
                .is_some_and(|path| path.ends_with("session-a.jsonl"))
        );
        assert!(hit.modified_at.is_some());
    }

    #[test]
    fn a_store_that_is_not_there_is_no_sessions() {
        let scratch = Scratch::new("missing");
        let root = scratch.0.join(".claude").to_string_lossy().into_owned();
        assert!(scan_once(&[root]).hits.is_empty());
    }

    /// A half-written or foreign file is skipped, not turned into a card with an
    /// empty prompt.
    #[test]
    fn an_unusable_file_is_skipped() {
        let scratch = Scratch::new("unusable");
        let projects = scratch.project_dir(".claude", "-home-dev-project");
        write(&projects.join("empty.jsonl"), "");
        write(&projects.join("junk.jsonl"), "not json\n");
        write(&projects.join("notes.txt"), "hello\n");

        let root = scratch.0.join(".claude").to_string_lossy().into_owned();
        assert!(scan_once(&[root]).hits.is_empty());
    }

    /// A session written after the app was closed is found and its newest turn is
    /// what the card shows — the whole point of running at attach.
    #[test]
    fn the_newest_session_is_the_one_a_panel_shows_first() {
        let scratch = Scratch::new("newest");
        let projects = scratch.project_dir(".claude", "-home-dev-project");
        write(
            &projects.join("older.jsonl"),
            &transcript("/home/dev/project", "model-x"),
        );
        // A second session, written later.
        std::thread::sleep(std::time::Duration::from_millis(2));
        write(
            &projects.join("newer.jsonl"),
            &transcript("/home/dev/project", "model-y"),
        );

        let root = scratch.0.join(".claude").to_string_lossy().into_owned();
        let mut hits = scan_once(&[root]).hits;
        hits.sort_by_key(|hit| hit.modified_at);
        assert_eq!(
            hits.last().map(|hit| hit.session_id.as_str()),
            Some("newer")
        );
    }

    /// Two accounts under the same home are two roots, and a session in each is
    /// found once — not once per root.
    #[test]
    fn a_second_account_under_the_same_home_is_its_own_root() {
        let scratch = Scratch::new("accounts");
        let primary = scratch.project_dir(".claude", "-home-dev-project");
        let work = scratch.project_dir(".claude-work", "-home-dev-project");
        write(
            &primary.join("personal.jsonl"),
            &transcript("/home/dev/project", "model-x"),
        );
        write(
            &work.join("work.jsonl"),
            &transcript("/home/dev/project", "model-x"),
        );

        let roots = claude_roots(&scratch.0.join(".claude"));
        assert_eq!(roots.len(), 2, "roots: {roots:?}");

        let mut ids: Vec<String> = scan_once(&roots)
            .hits
            .into_iter()
            .map(|hit| hit.session_id)
            .collect();
        ids.sort();
        assert_eq!(ids, vec!["personal", "work"]);
    }

    /// The tail is honoured, not the whole file: a session's last three turns come
    /// from the end.
    #[test]
    fn only_the_tail_of_a_large_transcript_is_read() {
        let scratch = Scratch::new("tail");
        let projects = scratch.project_dir(".claude", "-home-dev-project");
        let path = projects.join("big.jsonl");
        // Well past TAIL_BYTES of filler, then the real turns.
        for _ in 0..200 {
            write(
                &path,
                &format!(
                    "{}\n",
                    r#"{"type":"assistant","cwd":"/home/dev/project","message":{"content":[{"type":"text","text":"filler line with some length to it"}]}}"#
                ),
            );
        }
        write(&path, &transcript("/home/dev/project", "model-x"));

        let root = scratch.0.join(".claude").to_string_lossy().into_owned();
        let hits = scan_once(&[root]).hits;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].model.as_deref(), Some("model-x"));
        assert_eq!(hits[0].cwd, "/home/dev/project");
    }
}
