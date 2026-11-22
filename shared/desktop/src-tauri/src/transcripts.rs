//! Live transcript tails (#192): one reading position per session, from the
//! path the hook reported. The JSONL is parsed by `morrow-core::transcript`;
//! this module only owns the file position and the attach-time backfill.
//! The shell polls these tails while the panel is visible and drops them when
//! their session goes away.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use morrow_core::transcript::{ScanDelta, TailState, scan_tail_for_attach};

/// How much of an existing file an attach reads for its first reply.
const BACKFILL_BYTES: u64 = 64 * 1024;
/// How much one poll reads, so a backlog cannot stall the feed thread. What
/// is left is picked up by the next poll from the advanced offset.
const POLL_BYTES: u64 = 256 * 1024;

#[derive(Debug, Default)]
pub struct Tails {
    attached: HashMap<String, Attached>,
}

#[derive(Debug)]
struct Attached {
    path: PathBuf,
    state: TailState,
}

impl Tails {
    /// Attach (or re-attach when the hook reports a new path) and read the
    /// existing tail once. Nothing after the scan's `end_offset` is history:
    /// it belongs to the live tail.
    pub fn attach(&mut self, session_id: &str, path: &str) -> Option<ScanDelta> {
        if self
            .attached
            .get(session_id)
            .is_some_and(|attached| attached.path == Path::new(path))
        {
            return None;
        }
        let path = PathBuf::from(path);
        let (size, tail) = read_tail(&path)?;
        let scan = scan_tail_for_attach(&tail, size.saturating_sub(tail.len() as u64), size);
        self.attached.insert(
            session_id.to_owned(),
            Attached {
                path,
                state: TailState::at_offset(scan.end_offset),
            },
        );
        (!scan.delta.is_empty()).then_some(scan.delta)
    }

    /// The bytes appended since the last read, parsed by the core parser.
    /// `None` means "nothing new" as well as "the file is not readable".
    pub fn poll(&mut self, session_id: &str) -> Option<ScanDelta> {
        let attached = self.attached.get_mut(session_id)?;
        let mut file = File::open(&attached.path).ok()?;
        if file.metadata().ok()?.len() < attached.state.offset() {
            attached.state.truncated();
        }
        file.seek(SeekFrom::Start(attached.state.offset())).ok()?;
        let mut appended = Vec::new();
        file.take(POLL_BYTES).read_to_end(&mut appended).ok()?;
        attached.state.receive(&appended).map(|tail| tail.delta)
    }

    /// A session that disappeared must not be tailed any more.
    pub fn remove(&mut self, session_id: &str) {
        self.attached.remove(session_id);
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&str) -> bool) {
        self.attached.retain(|session_id, _| keep(session_id));
    }

    #[cfg(test)]
    pub fn has(&self, session_id: &str) -> bool {
        self.attached.contains_key(session_id)
    }
}

/// The last `BACKFILL_BYTES` of a file, plus its full size for the core's
/// attach-time scan. An unreadable file is simply not attached yet.
fn read_tail(path: &Path) -> Option<(u64, Vec<u8>)> {
    let mut file = File::open(path).ok()?;
    let size = file.metadata().ok()?.len();
    let start = size.saturating_sub(BACKFILL_BYTES);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut tail = Vec::new();
    file.take(BACKFILL_BYTES).read_to_end(&mut tail).ok()?;
    Some((size, tail))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    const PROMPT: &str = r#"{"type":"user","message":{"content":"Add filters"}}"#;
    const REPLY: &str =
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Adding filters"}]}}"#;
    const NEXT_REPLY: &str =
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Wired the bar"}]}}"#;

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!(
                "morrow-192-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&dir).expect("scratch dir");
            Self(dir)
        }

        fn file(&self) -> PathBuf {
            self.0.join("transcript.jsonl")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_lines(path: &Path, lines: &[&str]) {
        let mut text = lines.join("\n");
        text.push('\n');
        fs::write(path, text).expect("write transcript");
    }

    fn append_lines(path: &Path, lines: &[&str]) {
        let mut file = OpenOptions::new()
            .append(true)
            .open(path)
            .expect("open for append");
        for line in lines {
            writeln!(file, "{line}").expect("append line");
        }
    }

    #[test]
    fn attaching_backfills_the_tail_and_polling_reads_only_new_bytes() {
        let scratch = Scratch::new();
        let path = scratch.file();
        write_lines(&path, &[PROMPT, REPLY]);
        let mut tails = Tails::default();

        let backfill = tails
            .attach("a", path.to_str().unwrap())
            .expect("backfill of the existing tail");
        assert_eq!(backfill.last_user_prompt.as_deref(), Some("Add filters"));
        assert_eq!(
            backfill.last_assistant_message.as_deref(),
            Some("Adding filters")
        );
        assert!(
            tails.poll("a").is_none(),
            "attach must not re-read what it backfilled"
        );

        append_lines(&path, &[NEXT_REPLY]);
        let delta = tails.poll("a").expect("newly appended line");
        assert_eq!(
            delta.last_assistant_message.as_deref(),
            Some("Wired the bar")
        );
        assert!(
            tails.poll("a").is_none(),
            "the same bytes must not be parsed twice"
        );
    }

    #[test]
    fn an_unterminated_row_waits_for_its_newline() {
        let scratch = Scratch::new();
        let path = scratch.file();
        write_lines(&path, &[PROMPT]);
        let mut tails = Tails::default();
        tails.attach("a", path.to_str().unwrap());

        let mut file = OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open for append");
        write!(file, "{NEXT_REPLY}").expect("write a partial row");
        drop(file);
        assert!(
            tails.poll("a").is_none(),
            "a partial row is not a message yet"
        );

        append_lines(&path, &[""]);
        let delta = tails.poll("a").expect("the completed row");
        assert_eq!(
            delta.last_assistant_message.as_deref(),
            Some("Wired the bar")
        );
    }

    #[test]
    fn a_shorter_replaced_file_is_read_from_its_start() {
        let scratch = Scratch::new();
        let path = scratch.file();
        write_lines(&path, &[PROMPT, REPLY]);
        let mut tails = Tails::default();
        tails.attach("a", path.to_str().unwrap());

        write_lines(&path, &[NEXT_REPLY]);
        let delta = tails.poll("a").expect("the replacement's history");
        assert_eq!(
            delta.last_assistant_message.as_deref(),
            Some("Wired the bar")
        );
    }

    #[test]
    fn removing_a_session_stops_its_tail() {
        let scratch = Scratch::new();
        let path = scratch.file();
        write_lines(&path, &[REPLY]);
        let mut tails = Tails::default();
        tails.attach("a", path.to_str().unwrap());
        tails.remove("a");
        assert!(!tails.has("a"));

        append_lines(&path, &[NEXT_REPLY]);
        assert!(tails.poll("a").is_none(), "a gone session has no tail");
    }

    #[test]
    fn a_new_path_replaces_the_old_attachment() {
        let scratch = Scratch::new();
        let first = scratch.0.join("first.jsonl");
        let second = scratch.0.join("second.jsonl");
        write_lines(&first, &[REPLY]);
        write_lines(&second, &[NEXT_REPLY]);
        let mut tails = Tails::default();
        tails.attach("a", first.to_str().unwrap());

        let backfill = tails
            .attach("a", second.to_str().unwrap())
            .expect("backfill from the new path");
        assert_eq!(
            backfill.last_assistant_message.as_deref(),
            Some("Wired the bar")
        );
        assert!(tails.poll("a").is_none());
    }
}
