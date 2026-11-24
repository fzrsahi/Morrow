//! Read-only access to the SQLite session stores (#247c).
//!
//! Codex keeps `state_5.sqlite`, OpenCode and MiMo Code keep their own store.
//! Discovery only ever reads them, and the "only ever" is the whole difficulty:
//! a single write — even a write connection that is closed again — checkpoints
//! into the agent's own database while the agent is using it.
//!
//! ## The `immutable` fallback
//!
//! A WAL database whose `-wal` and `-shm` files are gone cannot be read by a
//! read-only connection: it may not create them, so every statement fails with
//! `SQLITE_CANTOPEN`. Codex leaves `state_5.sqlite` exactly like that whenever
//! its last connection closes — an idle Codex Desktop. With no writer on it the
//! file holds the whole database, so it is then opened `immutable`, which reads
//! it without creating, locking or changing anything.
//!
//! The fallback is not for every failure. `SQLITE_BUSY` and `SQLITE_LOCKED`
//! mean a writer **is** active — the opposite of "no writer" — so the store is
//! skipped for this scan instead. `immutable` would ignore that writer's
//! `-wal` pages and could read a stale value or a torn page mid-checkpoint.
//!
//! Opening is lazy in SQLite: the first statement is where the WAL index is
//! needed, so that is where the fallback has to be decided.

use std::path::Path;

use rusqlite::{Connection, OpenFlags};

/// A store that could not be opened, or a statement that could not run.
///
/// `Missing` is separate from the rest because it is the ordinary case — an
/// agent the user does not have — and must never be reported as a failure.
#[derive(Debug, PartialEq, Eq)]
pub enum StoreError {
    /// The file is not there: nothing to read, not an error to surface.
    Missing,
    /// A writer holds the store right now. Skip it this scan and try again:
    /// reading the raw file would ignore the writer's uncommitted `-wal` pages.
    Busy,
    /// The path cannot be expressed as a UTF-8 SQLite URI, so `immutable`
    /// cannot be tried. Never rewrite the name lossily: that would open a
    /// different file (or none at all) and call a fine store unreadable.
    NonUtf8Path,
    /// There is a file but SQLite will not open or read it.
    Unreadable(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => write!(formatter, "no store at that path"),
            Self::Busy => write!(formatter, "store is being written; try again next scan"),
            Self::NonUtf8Path => write!(formatter, "store path is not valid UTF-8"),
            Self::Unreadable(reason) => write!(formatter, "store unreadable: {reason}"),
        }
    }
}

impl std::error::Error for StoreError {}

/// Which way the store ended up being read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenMode {
    /// An ordinary read-only connection.
    ReadOnly,
    /// `immutable=1`: no locking, no sidecar files.
    Immutable,
}

/// Open `path` read-only, falling back to `immutable` when the ordinary
/// read-only open cannot read the store (#247c).
///
/// The fallback is triggered by any failure of the first statement **except**
/// `SQLITE_BUSY` / `SQLITE_LOCKED`, which are reported as [`StoreError::Busy`]
/// so the caller can skip a live store instead. The condition being detected
/// is "this store cannot be read the ordinary read-only way", and SQLite's code
/// for that is not stable across versions — the same missing-WAL state answers
/// `SQLITE_CANTOPEN` on some builds and `SQLITE_IOERR` on others (measured, see
/// the tests). Naming one of them would make the fallback dead code on the
/// other.
pub fn open_read_only(path: &Path) -> Result<Connection, StoreError> {
    open_reporting_mode(path).map(|(connection, _)| connection)
}

/// `open_read_only`, also saying which way it went. Exists so the fallback can
/// be asserted instead of assumed.
pub fn open_reporting_mode(path: &Path) -> Result<(Connection, OpenMode), StoreError> {
    if !path.exists() {
        return Err(StoreError::Missing);
    }
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let direct = Connection::open_with_flags(path, flags).map_err(open_error)?;
    match probe(&direct) {
        Ok(()) => Ok((direct, OpenMode::ReadOnly)),
        // A writer is active: the opposite of the state that makes `immutable`
        // safe. Leave the store alone and let the next scan try again.
        Err(error) if busy_or_locked(&error) => Err(StoreError::Busy),
        Err(direct_error) => {
            drop(direct);
            match open_immutable(path) {
                Ok(connection) => Ok((connection, OpenMode::Immutable)),
                // Neither way worked: report the ordinary error, which is the
                // one that describes the store rather than the fallback.
                Err(_) => Err(StoreError::Unreadable(direct_error.to_string())),
            }
        }
    }
}

/// `SQLITE_BUSY` and `SQLITE_LOCKED` say a writer is holding the store, not
/// that it cannot be read. Those must never send discovery to `immutable`.
fn busy_or_locked(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(error, _)
            if matches!(
                error.code,
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
            )
    )
}

fn open_error(error: rusqlite::Error) -> StoreError {
    if busy_or_locked(&error) {
        StoreError::Busy
    } else {
        StoreError::Unreadable(error.to_string())
    }
}

/// `file:…?immutable=1`: SQLite reads the file as it stands, with no locking
/// and no sidecar files. Only safe because a store in this state has no writer.
fn open_immutable(path: &Path) -> Result<Connection, StoreError> {
    let uri = immutable_uri(path)?;
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY
        | OpenFlags::SQLITE_OPEN_NO_MUTEX
        | OpenFlags::SQLITE_OPEN_URI;
    let connection = Connection::open_with_flags(uri, flags).map_err(open_error)?;
    probe(&connection).map_err(open_error)?;
    Ok(connection)
}

/// The `file:…?immutable=1` URI for `path`.
///
/// SQLite decodes `%HH` in the path of a URI, so every byte outside the
/// unreserved set must be escaped — including a literal `%`, which would
/// otherwise turn `100%2Fdone` into `100/done`. A non-UTF-8 name is refused
/// rather than lossily rewritten into a path that does not exist.
fn immutable_uri(path: &Path) -> Result<String, StoreError> {
    let Some(path) = path.to_str() else {
        return Err(StoreError::NonUtf8Path);
    };
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut uri = String::from("file:");
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            uri.push(byte as char);
        } else {
            uri.push('%');
            uri.push(HEX[(byte >> 4) as usize] as char);
            uri.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }
    uri.push_str("?immutable=1");
    Ok(uri)
}

/// The first statement, which is where a lazily-opened connection actually
/// opens — and therefore where a missing WAL index shows up.
fn probe(connection: &Connection) -> rusqlite::Result<()> {
    // An aggregate returns a row even on an empty schema. The question is
    // "did the connection open and can it execute?", not "are there tables?" —
    // a just-created store with no migrations yet is still readable.
    connection.query_row("SELECT count(*) FROM sqlite_master;", [], |_| Ok(()))?;
    Ok(())
}

/// The column names of `table_name`, read from `PRAGMA table_info`.
///
/// The agent stores change shape between versions — a column appears, another is
/// renamed — and a reader that assumed one layout would answer "nothing" on the
/// version it did not expect. Asking first is what lets a query adapt instead of
/// failing.
///
/// The name is not interpolated blindly: `PRAGMA` cannot take a bound parameter,
/// so a name with anything but identifier characters is refused rather than
/// trusted.
pub fn table_columns(connection: &Connection, table_name: &str) -> Vec<String> {
    if table_name.is_empty()
        || !table_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Vec::new();
    }
    let sql = format!("PRAGMA table_info({table_name});");
    let Ok(mut statement) = connection.prepare(&sql) else {
        return Vec::new();
    };
    let Ok(mut rows) = statement.query([]) else {
        return Vec::new();
    };
    let mut columns = Vec::new();
    while let Ok(Some(row)) = rows.next() {
        // Column 1 is `name`.
        if let Ok(name) = row.get::<_, String>(1) {
            columns.push(name);
        }
    }
    columns
}

/// A text column, trimmed, or `None` when it is absent, `NULL` or blank.
///
/// Every store uses an empty string where it means "unset", so a caller that
/// only checked for `NULL` would carry `Some("")` into a card and show a blank
/// model instead of falling back to the transcript.
pub fn non_empty_text(row: &rusqlite::Row<'_>, index: usize) -> Option<String> {
    let text: Option<String> = row.get(index).ok()?;
    let text = text?;
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("morrow-sqlite-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("a scratch directory");
            Self(path)
        }

        fn db(&self) -> PathBuf {
            self.0.join("store.sqlite")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn seed(path: &Path) {
        let connection = Connection::open(path).expect("a writable store");
        connection
            .execute_batch(
                "CREATE TABLE session (id TEXT PRIMARY KEY, directory TEXT);
                 INSERT INTO session VALUES ('s1', '/home/dev/project');",
            )
            .expect("a schema");
    }

    #[test]
    fn a_missing_store_is_missing_not_unreadable() {
        let scratch = Scratch::new("missing");
        assert_eq!(
            open_read_only(&scratch.db()).unwrap_err(),
            StoreError::Missing
        );
    }

    #[test]
    fn a_plain_store_opens_and_reads() {
        let scratch = Scratch::new("plain");
        seed(&scratch.db());

        let connection = open_read_only(&scratch.db()).expect("a read-only connection");
        let directory: String = connection
            .query_row(
                "SELECT directory FROM session WHERE id = 's1';",
                [],
                |row| row.get(0),
            )
            .expect("the row");
        assert_eq!(directory, "/home/dev/project");
    }

    /// A store whose schema is still empty — created before the first migration
    /// — is readable. "No rows in sqlite_master" is not "cannot read".
    #[test]
    fn an_empty_schema_opens_read_only_without_the_fallback() {
        let scratch = Scratch::new("empty-schema");
        {
            let _connection = Connection::open(scratch.db()).expect("a writable store");
        }

        let (connection, mode) =
            open_reporting_mode(&scratch.db()).expect("a readable, empty store");
        assert_eq!(
            mode,
            OpenMode::ReadOnly,
            "an empty schema is not unreadable"
        );
        let count: i64 = connection
            .query_row("SELECT count(*) FROM sqlite_master;", [], |row| row.get(0))
            .expect("a count");
        assert_eq!(count, 0);
    }

    /// The reason this module exists. A read-only connection must not be able to
    /// write, and the attempt must fail rather than succeed quietly.
    #[test]
    fn a_read_only_connection_cannot_write() {
        let scratch = Scratch::new("read-only");
        seed(&scratch.db());

        let connection = open_read_only(&scratch.db()).expect("a read-only connection");
        let error = connection
            .execute("INSERT INTO session VALUES ('s2', '/tmp');", [])
            .expect_err("a write must be refused");
        assert!(
            error.to_string().to_lowercase().contains("readonly")
                || error.to_string().to_lowercase().contains("read-only"),
            "unexpected error: {error}"
        );

        // And the file is unchanged, which is the part that matters.
        let writable = Connection::open(scratch.db()).expect("a writable store");
        let count: i64 = writable
            .query_row("SELECT COUNT(*) FROM session;", [], |row| row.get(0))
            .expect("a count");
        assert_eq!(count, 1);
    }

    /// A store created in WAL mode reads fine once its connection closed and
    /// SQLite checkpointed it — the ordinary case, and the one that must not
    /// regress into "unreadable" because the header still says WAL.
    #[test]
    fn a_checkpointed_wal_store_opens_read_only() {
        let scratch = Scratch::new("wal-checkpointed");
        let path = scratch.db();
        {
            let connection = Connection::open(&path).expect("a writable store");
            connection
                .execute_batch(
                    "PRAGMA journal_mode=WAL;
                     CREATE TABLE session (id TEXT PRIMARY KEY, directory TEXT);
                     INSERT INTO session VALUES ('s1', '/home/dev/project');",
                )
                .expect("a schema");
        }

        let (connection, mode) = open_reporting_mode(&path).expect("a read-only connection");
        assert_eq!(mode, OpenMode::ReadOnly, "the fallback is not needed here");
        let directory: String = connection
            .query_row(
                "SELECT directory FROM session WHERE id = 's1';",
                [],
                |row| row.get(0),
            )
            .expect("the row");
        assert_eq!(directory, "/home/dev/project");
    }

    /// BUSY/LOCKED are not "cannot read" — they say a writer is live, which is
    /// exactly when `immutable` would be unsafe. Skip the store for this scan.
    #[test]
    fn a_store_held_by_a_writer_is_skipped_not_opened_immutable() {
        let scratch = Scratch::new("busy");
        let path = scratch.db();
        seed(&path);
        let writer = Connection::open(&path).expect("a writable store");
        writer
            .execute_batch("BEGIN EXCLUSIVE;")
            .expect("an exclusive writer");

        assert_eq!(
            open_reporting_mode(&path).map(|(_, mode)| mode),
            Err(StoreError::Busy),
            "a live writer must be retried next scan, not read without locking"
        );
        drop(writer);

        // The same store is readable the ordinary way once the writer is gone.
        let (_, mode) = open_reporting_mode(&path).expect("a read-only connection");
        assert_eq!(mode, OpenMode::ReadOnly);
    }

    /// `immutable=1` reads a store without locking, without creating sidecar
    /// files, and without changing anything. Tested directly because the state
    /// that *needs* it could not be reproduced on this host (below).
    #[test]
    fn the_immutable_mode_reads_without_touching_the_store() {
        let scratch = Scratch::new("immutable");
        seed(&scratch.db());
        let before = fs::read(scratch.db()).expect("the store");
        let modified_before = fs::metadata(scratch.db())
            .and_then(|metadata| metadata.modified())
            .expect("an mtime");

        let connection = open_immutable(&scratch.db()).expect("an immutable connection");
        let directory: String = connection
            .query_row(
                "SELECT directory FROM session WHERE id = 's1';",
                [],
                |row| row.get(0),
            )
            .expect("the row");
        assert_eq!(directory, "/home/dev/project");
        drop(connection);

        assert_eq!(fs::read(scratch.db()).expect("the store"), before);
        assert_eq!(
            fs::metadata(scratch.db())
                .and_then(|metadata| metadata.modified())
                .expect("an mtime"),
            modified_before,
            "an immutable read must not write to the store"
        );
        // No `-wal`/`-shm` were created either.
        assert!(!scratch.0.join("store.sqlite-wal").exists());
        assert!(!scratch.0.join("store.sqlite-shm").exists());
    }

    /// SQLite decodes `%HH` in a `file:` URI, so a literal `%` must be encoded
    /// too: `100%2Fdone` would otherwise resolve to `100/done`.
    #[test]
    fn a_percent_in_the_path_is_not_decoded_as_an_escape() {
        let scratch = Scratch::new("percent");
        let dir = scratch.0.join("100%2Fdone");
        fs::create_dir_all(&dir).expect("a directory with a percent");
        let path = dir.join("store.sqlite");
        seed(&path);
        assert!(
            immutable_uri(&path)
                .expect("a URI")
                .contains("100%252Fdone"),
            "the literal % must be encoded, not left for SQLite to decode"
        );

        let connection = open_immutable(&path).expect("an immutable connection");
        let directory: String = connection
            .query_row(
                "SELECT directory FROM session WHERE id = 's1';",
                [],
                |row| row.get(0),
            )
            .expect("the row");
        assert_eq!(directory, "/home/dev/project");
    }

    /// `?` starts the URI query and `#` the fragment; both must stay in the
    /// path after the rewrite, not become part of the URI syntax.
    #[test]
    fn a_question_mark_and_a_hash_stay_in_the_path() {
        let scratch = Scratch::new("uri-specials");
        let path = scratch.0.join("store?#1.sqlite");
        seed(&path);

        let connection = open_immutable(&path).expect("an immutable connection");
        let directory: String = connection
            .query_row(
                "SELECT directory FROM session WHERE id = 's1';",
                [],
                |row| row.get(0),
            )
            .expect("the row");
        assert_eq!(directory, "/home/dev/project");
    }

    /// Lossy conversion would turn a non-UTF-8 name into a different path and
    /// then read that other file (or report the store unreadable). Refuse it
    /// before touching the filesystem. No file is created here: SQLite on this
    /// host cannot open such a name anyway, which is part of the point.
    #[cfg(unix)]
    #[test]
    fn a_non_utf8_path_is_not_lossily_rewritten() {
        use std::os::unix::ffi::OsStrExt;

        let scratch = Scratch::new("non-utf8");
        let path = scratch
            .0
            .join(std::ffi::OsStr::from_bytes(b"store-\xff.sqlite"));

        assert_eq!(open_immutable(&path).unwrap_err(), StoreError::NonUtf8Path);
    }

    /// What could **not** be proven here, recorded rather than hidden.
    ///
    /// `SQLiteReader.swift` says a WAL database whose `-wal` and `-shm` are gone
    /// "can't be read by a read-only connection: it may not create them, so
    /// every statement fails with `SQLITE_CANTOPEN`". That was measured on the
    /// SQLite Codex ships against. This host's SQLite (0.32.1 / bundled) answers
    /// differently in both directions:
    ///
    /// - after a clean close with the sidecars removed, the plain read-only open
    ///   **succeeds** (so nothing needs a fallback);
    /// - with a `-wal` left behind holding uncommitted-to-main data and the
    ///   `-shm` deleted, it fails with `SQLITE_IOERR` (`disk I/O error`), **not**
    ///   `SQLITE_CANTOPEN` — which is why the fallback still takes any
    ///   non-BUSY probe failure rather than a named code.
    ///
    /// So the fallback is a safety net for the state Swift observed, not
    /// something this machine can trigger. Pinned here so a future reader knows
    /// the branch is deliberate and where its evidence stops.
    #[test]
    fn a_wal_store_without_sidecars_reads_either_way() {
        let scratch = Scratch::new("wal-no-sidecars");
        let path = scratch.db();
        {
            let connection = Connection::open(&path).expect("a writable store");
            connection
                .execute_batch(
                    "PRAGMA journal_mode=WAL;
                     CREATE TABLE session (id TEXT PRIMARY KEY, directory TEXT);
                     INSERT INTO session VALUES ('s1', '/home/dev/project');",
                )
                .expect("a schema");
        }
        let _ = fs::remove_file(scratch.0.join("store.sqlite-wal"));
        let _ = fs::remove_file(scratch.0.join("store.sqlite-shm"));

        // Whichever way it goes, the answer is the data — not an error.
        let (connection, _mode) = open_reporting_mode(&path).expect("a read-only connection");
        let directory: String = connection
            .query_row(
                "SELECT directory FROM session WHERE id = 's1';",
                [],
                |row| row.get(0),
            )
            .expect("the row");
        assert_eq!(directory, "/home/dev/project");
    }

    #[test]
    fn table_columns_lists_what_the_store_actually_has() {
        let scratch = Scratch::new("columns");
        seed(&scratch.db());

        let connection = open_read_only(&scratch.db()).expect("a read-only connection");
        assert_eq!(
            table_columns(&connection, "session"),
            vec!["id", "directory"]
        );
        // A table that is not there answers nothing rather than failing.
        assert!(table_columns(&connection, "nope").is_empty());
    }

    /// `PRAGMA` cannot take a bound parameter, so the name is interpolated. A
    /// name carrying anything else is refused instead of being trusted.
    #[test]
    fn a_table_name_that_is_not_an_identifier_is_refused() {
        let scratch = Scratch::new("injection");
        seed(&scratch.db());

        let connection = open_read_only(&scratch.db()).expect("a read-only connection");
        for name in [
            "session; DROP TABLE session",
            "session)",
            "sess ion",
            "",
            "session'",
        ] {
            assert!(
                table_columns(&connection, name).is_empty(),
                "refused: {name}"
            );
        }
        // The table is still there, which is the point of refusing.
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM session;", [], |row| row.get(0))
            .expect("a count");
        assert_eq!(count, 1);
    }

    #[test]
    fn a_blank_text_column_is_not_a_value() {
        let scratch = Scratch::new("blank");
        {
            let connection = Connection::open(scratch.db()).expect("a writable store");
            connection
                .execute_batch(
                    "CREATE TABLE session (id TEXT, model TEXT, note TEXT);
                     INSERT INTO session VALUES ('s1', '  gpt-5  ', NULL);
                     INSERT INTO session VALUES ('s2', '   ', '   ');",
                )
                .expect("a schema");
        }
        let connection = open_read_only(&scratch.db()).expect("a read-only connection");
        let mut statement = connection
            .prepare("SELECT model, note FROM session ORDER BY id;")
            .expect("a statement");
        let mut rows = statement.query([]).expect("rows");

        let first = rows.next().expect("a row").expect("a row");
        assert_eq!(non_empty_text(first, 0), Some("gpt-5".to_owned()));
        assert_eq!(non_empty_text(first, 1), None);

        let second = rows.next().expect("a row").expect("a row");
        assert_eq!(non_empty_text(second, 0), None);
        assert_eq!(non_empty_text(second, 1), None);
    }
}
