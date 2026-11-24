//! Which file events are worth a discovery scan, and how often one may run (#259).
//!
//! The port's starting position was "rescan on every tick", and a scan reads the
//! stores under `~`. An agent writing its own transcript therefore bought a full
//! scan roughly every two seconds — for a file discovery is already following
//! line by line. The writes that matter are the ones to the stores nobody is
//! tailing: a new session appearing, a lease, a WAL side file. Those are counted;
//! the transcript being tailed is not.

use std::collections::{HashMap, HashSet};

/// Two scans closer together than this are one scan: the second waits.
pub const MINIMUM_GAP_MS: u64 = 5_000;

/// How many files one cache holds before it is cleared wholesale.
///
/// A cleared cache costs one extra read per file and nothing else, so the number
/// only has to be larger than the stores a real machine has: a heavy year of Claude
/// Code is a few thousand transcripts.
pub const SCAN_CACHE_CAPACITY: usize = 4_096;

/// Whether a changed path is news to discovery.
///
/// A path the tailer is already following is not: the tailer sees every line as
/// it lands, so a scan for it would read what is already on screen.
pub fn is_interesting(path: &str, tailed_paths: &HashSet<String>) -> bool {
    !tailed_paths.contains(path)
}

/// Whether any path in a batch is news.
pub fn has_interesting_event(paths: &[String], tailed_paths: &HashSet<String>) -> bool {
    paths.iter().any(|path| is_interesting(path, tailed_paths))
}

/// What to do about a batch of events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Scan now.
    ScanNow,
    /// Scan when the gap has passed; arm a timer for this instant.
    ScanLater(u64),
    /// Nothing worth scanning.
    Ignore,
}

/// Leading + trailing throttle: a burst scans at once, then once more when the
/// quietest gap has passed.
///
/// The trailing scan is never lost, so the last lifecycle write of a burst is
/// always read — which is the one that says the turn ended.
#[derive(Debug, Default)]
pub struct ScanThrottle {
    last_scan_at_ms: Option<u64>,
    minimum_gap_ms: u64,
}

impl ScanThrottle {
    pub fn new(minimum_gap_ms: u64) -> Self {
        Self {
            last_scan_at_ms: None,
            minimum_gap_ms,
        }
    }

    /// What to do about a batch seen at `now_ms`.
    pub fn decide(&mut self, has_interesting_event: bool, now_ms: u64) -> Decision {
        if !has_interesting_event {
            return Decision::Ignore;
        }
        let Some(last) = self.last_scan_at_ms else {
            return Decision::ScanNow;
        };
        if now_ms.saturating_sub(last) >= self.minimum_gap_ms {
            return Decision::ScanNow;
        }
        Decision::ScanLater(last + self.minimum_gap_ms)
    }

    /// Record that a scan started; the next gap is measured from here.
    pub fn note_scan_started(&mut self, now_ms: u64) {
        self.last_scan_at_ms = Some(now_ms);
    }

    /// Forget the clock (watcher restart, stop).
    pub fn reset(&mut self) {
        self.last_scan_at_ms = None;
    }
}

/// A file's fingerprint: what it looked like the last time it was read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fingerprint {
    /// Nanoseconds since the Unix epoch, not milliseconds.
    ///
    /// Milliseconds looked sufficient and are not: a store rewritten with
    /// different content of the *same size* inside one millisecond has an
    /// identical millisecond fingerprint, so the scan would skip the write that
    /// just changed the session. Found by a test asserting a read count, which
    /// is why the count is the assertion rather than the value.
    pub modified_nanos: u64,
    pub size: u64,
}

/// Remembers which files a scan has already read, so an unchanged store is not
/// read again.
///
/// The key is `(mtime, size)`, which is what a store's own writes change. A file
/// rewritten with identical content and an identical size within the mtime
/// resolution of the filesystem would be missed — accepted, because the
/// alternative is hashing every store on every tick, which is the cost this
/// exists to avoid.
#[derive(Debug, Default)]
pub struct ScanCache {
    seen: HashMap<String, Fingerprint>,
    /// Bounded so a long-running session with many sessions cannot grow it
    /// without limit. Cleared wholesale when it is reached, since a cache miss
    /// costs one extra read and nothing else.
    capacity: usize,
}

impl ScanCache {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            seen: HashMap::new(),
            capacity,
        }
    }

    /// Whether `path` needs reading, recording the fingerprint if it does not.
    ///
    /// `Some(fingerprint)` means "unchanged, skip it". `None` means "read it",
    /// and the caller is expected to call [`remember`] once it has.
    pub fn unchanged(&self, path: &str, fingerprint: Fingerprint) -> Option<Fingerprint> {
        self.seen
            .get(path)
            .filter(|seen| **seen == fingerprint)
            .copied()
    }

    /// Record what a read saw.
    pub fn remember(&mut self, path: &str, fingerprint: Fingerprint) {
        if self.capacity > 0 && self.seen.len() >= self.capacity && !self.seen.contains_key(path) {
            self.seen.clear();
        }
        self.seen.insert(path.to_owned(), fingerprint);
    }

    /// Forget one path, so the next scan reads it whatever its fingerprint says.
    pub fn forget(&mut self, path: &str) {
        self.seen.remove(path);
    }

    /// How many stores are remembered. A test asserts this does not grow past
    /// the capacity; production uses it for diagnostics.
    pub fn len(&self) -> usize {
        self.seen.len()
    }

    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }
}

/// What one store read produced.
#[derive(Debug, PartialEq, Eq)]
pub enum StoreRead {
    /// The fingerprint matched: nothing was read.
    Unchanged,
    /// The store was read, and this is what it held.
    Read(Vec<u8>),
    /// There is no such file (or it cannot be stat'd): nothing to read.
    Missing,
}

/// The fingerprint of `path` as the filesystem reports it.
pub fn fingerprint_of(path: &std::path::Path) -> Option<Fingerprint> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    let modified_nanos = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        // Fits in `u64` until the year 2554; `as_nanos` is `u128`.
        .map(|elapsed| u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX))?;
    Some(Fingerprint {
        modified_nanos,
        size: metadata.len(),
    })
}

/// File text, kept across scans so an unchanged store is not read again (#284).
///
/// The cache lives **across ticks**, not inside one scan: a scan that built its own
/// cache would read everything and forget it. It is `&self` with a lock inside
/// because the readers are handed it by reference — several of them run in one
/// scan, and none of them owns it.
///
/// Keyed by path, and the value is kept only while the fingerprint matches. A store
/// rewritten with different content of the same size *and* the same nanosecond is
/// the one case this cannot see, which is why the fingerprint is nanoseconds and
/// not milliseconds (#259).
#[derive(Default)]
pub struct TextCache {
    inner: std::sync::Mutex<TextCacheInner>,
}

#[derive(Default)]
struct TextCacheInner {
    fingerprints: ScanCache,
    text: HashMap<String, std::sync::Arc<str>>,
    /// How many times a file was actually opened. A test asserts on this rather
    /// than on elapsed time: "no read happened" is a fact, "it was fast" is not.
    reads: usize,
}

impl TextCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: std::sync::Mutex::new(TextCacheInner {
                fingerprints: ScanCache::with_capacity(capacity),
                ..TextCacheInner::default()
            }),
        }
    }

    /// The file's whole text, or its last `limit` bytes when it is longer.
    ///
    /// `limit` of `None` means the whole file. The two limits a reader uses
    /// (a tail and a head) are different questions about the same file, so the
    /// head is a **separate** entry: caching one under the other's key would hand
    /// a reader the wrong end of the file.
    pub fn text(
        &self,
        path: &std::path::Path,
        limit: Option<u64>,
        from_end: bool,
    ) -> Option<std::sync::Arc<str>> {
        let key = format!(
            "{}|{}|{}",
            path.to_string_lossy(),
            limit.map(|limit| limit.to_string()).unwrap_or_default(),
            from_end
        );
        let Some(fingerprint) = fingerprint_of(path) else {
            let mut inner = self.lock();
            inner.fingerprints.forget(&key);
            inner.text.remove(&key);
            return None;
        };
        {
            let inner = self.lock();
            if inner.fingerprints.unchanged(&key, fingerprint).is_some() {
                return inner.text.get(&key).cloned();
            }
        }
        // `Arc<str>` rather than `String`: a cache that hands out copies is a cache
        // that still copies everything. Four thousand 64 KB clones was 250 MB of
        // memcpy — the whole of what was left of a scan (#284).
        let contents: std::sync::Arc<str> = read_window(path, limit, from_end)?.into();
        let mut inner = self.lock();
        inner.fingerprints.remember(&key, fingerprint);
        inner.reads += 1;
        inner.text.insert(key, std::sync::Arc::clone(&contents));
        Some(contents)
    }

    /// How many files this cache has opened since it was created.
    pub fn reads(&self) -> usize {
        self.lock().reads
    }

    /// Forget everything. A store that moved is a miss, not a stale answer.
    pub fn clear(&self) {
        let mut inner = self.lock();
        *inner = TextCacheInner::default();
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, TextCacheInner> {
        // A poisoned lock means another thread panicked while holding it; the
        // cache holds no invariant worth propagating that panic through.
        self.inner.lock().unwrap_or_else(|error| error.into_inner())
    }
}

fn read_window(path: &std::path::Path, limit: Option<u64>, from_end: bool) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let size = file.metadata().ok()?.len();
    let Some(limit) = limit else {
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).ok()?;
        return Some(String::from_utf8_lossy(&bytes).into_owned());
    };
    let start = if from_end {
        size.saturating_sub(limit)
    } else {
        0
    };
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::new();
    file.take(limit).read_to_end(&mut bytes).ok()?;
    if from_end && start > 0 {
        // Skip to the first newline: the row at the cut is half a line, and
        // dropping it keeps a cut multi-byte character from making the whole tail
        // lossy.
        if let Some(index) = bytes.iter().position(|byte| *byte == b'\n') {
            bytes.drain(..=index);
        }
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Read `path` unless its fingerprint says it has not changed since the last
/// read.
///
/// This is where the incremental promise is kept: an idle agent leaves its store
/// untouched, and its fingerprint is unchanged, so no bytes are read and no JSON
/// is parsed. `read` is only called when the file actually moved on.
pub fn read_if_changed(
    cache: &mut ScanCache,
    path: &std::path::Path,
    read: impl FnOnce(&std::path::Path) -> Option<Vec<u8>>,
) -> StoreRead {
    let Some(fingerprint) = fingerprint_of(path) else {
        // The file went away: forget it, so it is a read again if it returns.
        cache.forget(&path.to_string_lossy());
        return StoreRead::Missing;
    };
    let key = path.to_string_lossy().into_owned();
    if cache.unchanged(&key, fingerprint).is_some() {
        return StoreRead::Unchanged;
    }
    match read(path) {
        Some(contents) => {
            cache.remember(&key, fingerprint);
            StoreRead::Read(contents)
        }
        // Readable by stat but not by read: do not remember it, or the next tick
        // would skip a file that was never successfully read.
        None => StoreRead::Missing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tailed(paths: &[&str]) -> HashSet<String> {
        paths.iter().map(|path| (*path).to_owned()).collect()
    }

    fn fingerprint(modified_nanos: u64, size: u64) -> Fingerprint {
        Fingerprint {
            modified_nanos,
            size,
        }
    }

    #[test]
    fn a_path_the_tailer_follows_is_not_news() {
        let tailed = tailed(&["/home/dev/.claude/projects/p/abc.jsonl"]);
        assert!(!is_interesting(
            "/home/dev/.claude/projects/p/abc.jsonl",
            &tailed
        ));
        assert!(is_interesting(
            "/home/dev/.claude/projects/p/new.jsonl",
            &tailed
        ));
        assert!(is_interesting("", &tailed));
    }

    #[test]
    fn a_batch_is_news_when_any_of_its_paths_is() {
        let tailed = tailed(&["/home/dev/.claude/projects/p/abc.jsonl"]);
        assert!(!has_interesting_event(
            &["/home/dev/.claude/projects/p/abc.jsonl".to_owned()],
            &tailed
        ));
        assert!(has_interesting_event(
            &[
                "/home/dev/.claude/projects/p/abc.jsonl".to_owned(),
                "/home/dev/.codex/sessions/new.jsonl".to_owned(),
            ],
            &tailed
        ));
        assert!(!has_interesting_event(&[], &tailed));
    }

    #[test]
    fn a_batch_with_nothing_interesting_is_ignored() {
        let mut throttle = ScanThrottle::new(MINIMUM_GAP_MS);
        assert_eq!(throttle.decide(false, 1_000), Decision::Ignore);
        // Nothing was recorded, so the next real event still scans at once.
        assert_eq!(throttle.decide(true, 1_000), Decision::ScanNow);
    }

    #[test]
    fn the_first_interesting_batch_scans_at_once() {
        let mut throttle = ScanThrottle::new(MINIMUM_GAP_MS);
        assert_eq!(throttle.decide(true, 1_000), Decision::ScanNow);
    }

    /// The burst: one scan immediately, then one after the gap — the trailing
    /// scan is the one that reads the turn's final write.
    #[test]
    fn a_burst_scans_once_then_once_more_after_the_gap() {
        let mut throttle = ScanThrottle::new(MINIMUM_GAP_MS);
        assert_eq!(throttle.decide(true, 10_000), Decision::ScanNow);
        throttle.note_scan_started(10_000);

        assert_eq!(throttle.decide(true, 10_500), Decision::ScanLater(15_000));
        assert_eq!(throttle.decide(true, 13_000), Decision::ScanLater(15_000));
        // The armed timer fires at exactly the gap.
        assert_eq!(throttle.decide(true, 15_000), Decision::ScanNow);
        throttle.note_scan_started(15_000);

        assert_eq!(throttle.decide(true, 15_100), Decision::ScanLater(20_000));
    }

    #[test]
    fn a_reset_forgets_the_clock() {
        let mut throttle = ScanThrottle::new(MINIMUM_GAP_MS);
        assert_eq!(throttle.decide(true, 10_000), Decision::ScanNow);
        throttle.note_scan_started(10_000);
        assert_eq!(throttle.decide(true, 10_100), Decision::ScanLater(15_000));

        throttle.reset();
        assert_eq!(throttle.decide(true, 10_100), Decision::ScanNow);
    }

    /// A clock that went backwards (a suspend, a manual clock change) must not
    /// schedule a scan in the past and spin.
    #[test]
    fn a_clock_that_went_backwards_does_not_schedule_into_the_past() {
        let mut throttle = ScanThrottle::new(MINIMUM_GAP_MS);
        assert_eq!(throttle.decide(true, 100_000), Decision::ScanNow);
        throttle.note_scan_started(100_000);
        assert_eq!(throttle.decide(true, 50_000), Decision::ScanLater(105_000));
    }

    #[test]
    fn an_unchanged_store_is_not_read_again() {
        let mut cache = ScanCache::with_capacity(16);
        assert_eq!(cache.unchanged("/store", fingerprint(1_000, 64)), None);

        cache.remember("/store", fingerprint(1_000, 64));
        assert_eq!(
            cache.unchanged("/store", fingerprint(1_000, 64)),
            Some(fingerprint(1_000, 64))
        );

        // A write changes at least one of the two, and that is a read.
        assert_eq!(cache.unchanged("/store", fingerprint(1_001, 64)), None);
        assert_eq!(cache.unchanged("/store", fingerprint(1_000, 65)), None);
    }

    #[test]
    fn forgetting_a_path_forces_a_read() {
        let mut cache = ScanCache::with_capacity(16);
        cache.remember("/store", fingerprint(1_000, 64));
        cache.forget("/store");
        assert_eq!(cache.unchanged("/store", fingerprint(1_000, 64)), None);
        // Forgetting something that was never there is not an error.
        cache.forget("/never");
    }

    #[test]
    fn the_cache_does_not_grow_without_a_bound() {
        let mut cache = ScanCache::with_capacity(4);
        for index in 0..4 {
            cache.remember(&format!("/store{index}"), fingerprint(1, 1));
        }
        assert_eq!(cache.len(), 4);

        // The fifth evicts everything rather than growing: a miss costs one
        // read, so clearing is cheaper than a map that never shrinks.
        cache.remember("/store4", fingerprint(1, 1));
        assert_eq!(cache.len(), 1);

        // Re-remembering a known path never triggers the clear.
        let mut cache = ScanCache::with_capacity(2);
        cache.remember("/a", fingerprint(1, 1));
        cache.remember("/b", fingerprint(1, 1));
        cache.remember("/a", fingerprint(2, 1));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn a_cache_with_no_capacity_still_answers() {
        let mut cache = ScanCache::with_capacity(0);
        cache.remember("/store", fingerprint(1, 64));
        assert_eq!(
            cache.unchanged("/store", fingerprint(1, 64)),
            Some(fingerprint(1, 64))
        );
        assert!(!cache.is_empty());
    }

    /// The AC of #259: the count of reads, not just the answer. An idle store
    /// must cost zero reads on every tick after the first.
    #[test]
    fn an_idle_store_is_read_once_and_then_never_again() {
        use std::cell::Cell;

        let dir = std::env::temp_dir().join(format!("morrow-scan-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        let path = dir.join("store.jsonl");
        std::fs::write(&path, b"{\"a\":1}\n").expect("a store");

        let reads = Cell::new(0);
        let mut cache = ScanCache::with_capacity(16);
        let tick = |cache: &mut ScanCache| {
            read_if_changed(cache, &path, |path| {
                reads.set(reads.get() + 1);
                std::fs::read(path).ok()
            })
        };

        assert!(matches!(tick(&mut cache), StoreRead::Read(_)));
        assert_eq!(reads.get(), 1);

        // Five idle ticks: the fingerprint never changed.
        for _ in 0..5 {
            assert_eq!(tick(&mut cache), StoreRead::Unchanged);
        }
        assert_eq!(reads.get(), 1, "an idle store must not be read again");

        // The agent writes: one read, not six. Same size, different content —
        // the case a millisecond fingerprint could not see.
        let before = fingerprint_of(&path).expect("a fingerprint");
        std::fs::write(&path, b"{\"a\":2}\n").expect("a write");
        assert_ne!(
            before,
            fingerprint_of(&path).expect("a fingerprint"),
            "the filesystem must report this write, or the premise is void"
        );
        assert!(matches!(tick(&mut cache), StoreRead::Read(_)));
        assert_eq!(reads.get(), 2);

        // The file goes away, then returns: it is read again.
        std::fs::remove_file(&path).expect("a remove");
        assert_eq!(tick(&mut cache), StoreRead::Missing);
        std::fs::write(&path, b"{\"a\":3}\n").expect("a write");
        assert!(matches!(tick(&mut cache), StoreRead::Read(_)));
        assert_eq!(reads.get(), 3);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_store_that_cannot_be_read_is_not_remembered_as_read() {
        let dir =
            std::env::temp_dir().join(format!("morrow-scan-unreadable-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        let path = dir.join("store.jsonl");
        std::fs::write(&path, b"x").expect("a store");

        let mut cache = ScanCache::with_capacity(16);
        // stat succeeds, read fails.
        assert_eq!(
            read_if_changed(&mut cache, &path, |_| None),
            StoreRead::Missing
        );
        assert!(
            cache.is_empty(),
            "a failed read must not look like a good one"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
