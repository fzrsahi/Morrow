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

/// The age a transcript may have to become a backfill hit, and the age a
/// backfilled card may reach before the staleness sweep drops it (#261).
///
/// macOS gates discovery on a live `claude` process; this reader has no process
/// to gate on, so time is the substitute. A session last written a week ago is
/// history, not "what is happening now", and must not take a panel slot.
pub const BACKFILL_WINDOW_MS: u64 = 7 * 24 * 60 * 60 * 1000;

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
        self.read_at(root, now_ms())
    }
}

impl ClaudeTranscripts {
    /// `read` with the clock injected, so the recency cap can be tested without
    /// sleeping for a week or fabricating mtimes.
    fn read_at(&self, root: &str, now: u64) -> Vec<DiscoveryHit> {
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
                // No process to gate on at attach: a transcript that has not
                // been written recently is history, and becomes no card at all.
                if now.saturating_sub(file.modified_ms) > BACKFILL_WINDOW_MS {
                    continue;
                }
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
    let mut siblings: Vec<String> = Vec::new();
    // A `~/.claude-work` style sibling: same parent, holds `projects/`.
    if let Some(parent) = Path::new(&primary).parent() {
        if let Ok(entries) = std::fs::read_dir(parent) {
            siblings = entries
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
        }
    }
    // One call: `scan_roots` puts the primary first and drops duplicates.
    dedupe(scan::scan_roots(
        &primary,
        &siblings.iter().map(String::as_str).collect::<Vec<_>>(),
    ))
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
    use std::fs::{self, File, OpenOptions};
    use std::io::Write;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use super::*;

    /// A scratch `~` that removes itself, with a suffix unique enough that two
    /// tests named the same in different crates cannot share the directory.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos());
            let path = std::env::temp_dir().join(format!(
                "morrow-backfill-{}-{name}-{unique}",
                std::process::id()
            ));
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

    fn set_modified_ms(path: &Path, millis: u64) {
        let modified = UNIX_EPOCH + Duration::from_millis(millis);
        File::options()
            .write(true)
            .open(path)
            .expect("an openable transcript")
            .set_times(std::fs::FileTimes::new().set_modified(modified))
            .expect("a set mtime");
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

    /// Without a live process to gate on, age is the substitute: the tail of a
    /// session from before the window is history, not a card on attach (#261).
    #[test]
    fn only_a_recent_transcript_becomes_a_backfill_hit() {
        let scratch = Scratch::new("recency");
        let projects = scratch.project_dir(".claude", "-home-dev-project");
        let old = projects.join("old.jsonl");
        let recent = projects.join("recent.jsonl");
        write(&old, &transcript("/home/dev/project", "old-model"));
        write(&recent, &transcript("/home/dev/project", "recent-model"));
        let now = now_ms();
        set_modified_ms(&old, now - BACKFILL_WINDOW_MS - 1);
        set_modified_ms(&recent, now - BACKFILL_WINDOW_MS + 1_000);

        let root = scratch.0.join(".claude").to_string_lossy().into_owned();
        // The injected clock pins the boundary; `scan_once` then exercises the
        // same mtimes through the real `read` path.
        let at: Vec<_> = ClaudeTranscripts
            .read_at(&root, now)
            .into_iter()
            .map(|hit| hit.session_id)
            .collect();
        assert_eq!(at, ["recent"]);
        let scanned: Vec<_> = scan_once(&[root])
            .hits
            .into_iter()
            .map(|hit| hit.session_id)
            .collect();
        assert_eq!(scanned, ["recent"]);
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

    /// The cut branch actually runs: 600 lines of 140-ish bytes is well past
    /// `TAIL_BYTES`, so the marker at the top is outside the slice and the one
    /// in the last two turns is inside it.
    #[test]
    fn only_the_tail_of_a_large_transcript_is_read() {
        let scratch = Scratch::new("tail");
        let projects = scratch.project_dir(".claude", "-home-dev-project");
        let path = projects.join("big.jsonl");
        // Only the very first line carries this marker; the filler after it is
        // unmarked so a last-three-turns slice cannot smuggle the marker in.
        write(
            &path,
            concat!(
                r#"{"type":"assistant","cwd":"/home/dev/project","message":{"content":[{"type":"text","text":"FIRST_LINE_MARKER only at the top"}]}}"#,
                "\n"
            ),
        );
        for _ in 0..600 {
            write(
                &path,
                concat!(
                    r#"{"type":"assistant","cwd":"/home/dev/project","message":{"content":[{"type":"text","text":"filler line with some length to it"}]}}"#,
                    "\n"
                ),
            );
        }
        write(
            &path,
            &format!(
                "{}\n{}\n",
                r#"{"type":"user","cwd":"/home/dev/project","message":{"content":"TAIL_ONLY_MARKER user turn"}}"#,
                r#"{"type":"assistant","cwd":"/home/dev/project","message":{"model":"model-x","content":[{"type":"text","text":"TAIL_ONLY_MARKER reply"}]}}"#
            ),
        );

        let root = scratch.0.join(".claude").to_string_lossy().into_owned();
        let hits = scan_once(&[root]).hits;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].model.as_deref(), Some("model-x"));
        assert_eq!(hits[0].cwd, "/home/dev/project");
        let texts: Vec<&str> = hits[0]
            .recent_messages
            .iter()
            .map(|message| message.text.as_str())
            .collect();
        assert!(
            texts.iter().any(|text| text.contains("TAIL_ONLY_MARKER")),
            "the last turns must be read: {texts:?}"
        );
        assert!(
            texts.iter().all(|text| !text.contains("FIRST_LINE_MARKER")),
            "the beginning of the file is outside TAIL_BYTES: {texts:?}"
        );
    }
}
