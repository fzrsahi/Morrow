//! Pure parsers behind the Workspace tab (#462). The shell runs `git`; these
//! tests pin what the parsed rows mean, including the cases a hand-rolled
//! `split` on a path would get wrong.

use morrow_core::git::{GitCommit, GitStatusCounts, parse_log, parse_status};

#[test]
fn status_columns_are_counted_separately() {
    let output = [
        "M  src/main.rs",
        " M src/lib.rs",
        "?? notes.txt",
        "A  added.rs",
        " D removed.rs",
        "R  old.rs -> new.rs",
        "UU both.rs",
    ]
    .join("\n");
    assert_eq!(
        parse_status(&output),
        GitStatusCounts {
            staged: 4,
            modified: 3,
            untracked: 1,
        }
    );
}

#[test]
fn status_ignores_blank_and_short_rows() {
    assert_eq!(parse_status(""), GitStatusCounts::default());
    assert_eq!(parse_status("\n\n"), GitStatusCounts::default());
    assert_eq!(parse_status("?\nM\n"), GitStatusCounts::default());
    // A path containing spaces is still one row, not two changes.
    assert_eq!(
        parse_status(" M my changed file.rs\n"),
        GitStatusCounts {
            modified: 1,
            ..GitStatusCounts::default()
        }
    );
}

#[test]
fn log_reads_hash_and_subject_across_the_separator() {
    let output = "a1b2c3d\u{1f}fix: keep status\nfnn9e8d\u{1f}docs: explain: why\n";
    assert_eq!(
        parse_log(output, 5),
        vec![
            GitCommit {
                hash: "a1b2c3d".into(),
                subject: "fix: keep status".into(),
            },
            GitCommit {
                hash: "fnn9e8d".into(),
                subject: "docs: explain: why".into(),
            },
        ]
    );
}

#[test]
fn log_caps_the_list_and_drops_malformed_rows() {
    let output = "a\u{1f}one\nb\u{1f}two\nc\u{1f}three\n\nmalformed\nd\u{1f}\n";
    assert_eq!(parse_log(output, 2).len(), 2);
    assert!(parse_log(output, 0).is_empty());
    assert_eq!(parse_log("no separator\n", 5), Vec::new());
}
