use ggt_model::{Change, Oid};
use pretty_assertions::assert_eq;

use super::*;

const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn oid(hex: &str) -> Oid {
    Oid::parse(hex).unwrap()
}

// ---- details ----

#[test]
fn details_args_ask_for_the_metadata_of_one_commit() {
    assert_eq!(
        details_args(&oid(A)),
        [
            "show".to_owned(),
            "--no-patch".into(),
            "-z".into(),
            format!("--format={DETAILS_FORMAT}"),
            A.into(),
            "--".into()
        ]
    );
}

#[test]
fn parses_details_with_a_multi_line_body() {
    let out = format!(
        "{A}\x1f{B}\x1fAnn\x1fann@x.io\x1f100\x1fCom\x1fcom@x.io\x1f200\x1fFix login\n\nBody line 1\nwith \x1f odd bytes\n\0"
    );
    let details = parse_details(&out).unwrap();
    assert_eq!(details.commit.id, oid(A));
    assert_eq!(details.commit.parents, [oid(B)]);
    assert_eq!(details.commit.author.email, "ann@x.io");
    assert_eq!(details.commit.committer.time, 200);
    assert_eq!(details.commit.subject, "Fix login");
    assert_eq!(details.body(), "Body line 1\nwith \x1f odd bytes");
}

#[test]
fn root_commit_details_have_no_parents_and_maybe_no_body() {
    let out = format!("{A}\x1f\x1fAnn\x1fa@x\x1f1\x1fAnn\x1fa@x\x1f1\x1fInitial commit\n\0");
    let details = parse_details(&out).unwrap();
    assert!(details.commit.parents.is_empty());
    assert_eq!(details.body(), "");
}

#[test]
fn truncated_details_are_an_error() {
    assert!(matches!(
        parse_details(&format!("{A}\x1f{B}")),
        Err(ParseError::MissingField { .. })
    ));
    assert!(matches!(
        parse_details(""),
        Err(ParseError::MissingField { .. })
    ));
}

// ---- changed files ----

#[test]
fn files_args_diff_merges_against_the_first_parent_and_detect_renames() {
    assert_eq!(
        files_args(&oid(A)),
        [
            "show",
            "--diff-merges=first-parent",
            "--find-renames",
            "--raw",
            "--numstat",
            "-z",
            "--format=",
            A,
            "--"
        ]
    );
}

#[test]
fn parses_numstat_with_renames_binaries_and_odd_paths() {
    let out = "3\t1\tsrc/main.rs\0-\t-\tlogo.png\0\
               1\t0\t\0old name.txt\0new\tname.txt\0\
               0\t0\tdir é/文件.md\0";
    assert_eq!(
        parse_files(out).unwrap(),
        [
            FileChange {
                path: "src/main.rs".into(),
                old_path: None,
                status: Change::Modified,
                stat: Numstat::Lines {
                    added: 3,
                    deleted: 1
                },
            },
            FileChange {
                path: "logo.png".into(),
                old_path: None,
                status: Change::Modified,
                stat: Numstat::Binary,
            },
            FileChange {
                path: "new\tname.txt".into(),
                old_path: Some("old name.txt".into()),
                status: Change::Renamed,
                stat: Numstat::Lines {
                    added: 1,
                    deleted: 0
                },
            },
            FileChange {
                path: "dir é/文件.md".into(),
                old_path: None,
                status: Change::Modified,
                stat: Numstat::Lines {
                    added: 0,
                    deleted: 0
                },
            },
        ]
    );
}

#[test]
fn raw_records_give_each_file_its_status() {
    // `--raw --numstat -z`: every raw record, then every numstat record,
    // in the same order.
    let out = "\n:000000 100644 0000000 3e75765 A\0added.txt\0\
               :100644 000000 587be6b 0000000 D\0del.txt\0\
               :100644 100644 9405325 0fdf397 M\0keep.txt\0\
               :100644 100644 b566061 b566061 R100\0old.txt\0sub/new.txt\0\
               :100644 100644 b566061 b566061 C075\0a.txt\0b.txt\0\
               :100644 120000 1111111 2222222 T\0link\0\
               1\t0\tadded.txt\0\
               0\t1\tdel.txt\0\
               1\t0\tkeep.txt\0\
               0\t0\t\0old.txt\0sub/new.txt\0\
               2\t0\t\0a.txt\0b.txt\0\
               1\t1\tlink\0";
    let files = parse_files(out).unwrap();
    let got: Vec<(&str, Option<&str>, Change)> = files
        .iter()
        .map(|f| (f.path.as_str(), f.old_path.as_deref(), f.status))
        .collect();
    assert_eq!(
        got,
        [
            ("added.txt", None, Change::Added),
            ("del.txt", None, Change::Deleted),
            ("keep.txt", None, Change::Modified),
            ("sub/new.txt", Some("old.txt"), Change::Renamed),
            ("b.txt", Some("a.txt"), Change::Copied),
            ("link", None, Change::TypeChanged),
        ]
    );
    assert_eq!(
        files[0].stat,
        Numstat::Lines {
            added: 1,
            deleted: 0
        }
    );
}

#[test]
fn a_raw_record_without_its_path_is_an_error() {
    assert!(matches!(
        parse_files(":100644 100644 a b M\0"),
        Err(ParseError::MissingField { .. })
    ));
    assert!(matches!(
        parse_files(":100644 100644 a b R100\0only-old\0"),
        Err(ParseError::MissingField { .. })
    ));
}

#[test]
fn leading_newlines_between_records_are_ignored() {
    // `--format=` can leave an empty line before the numstat.
    let files = parse_files("\n1\t2\ta.txt\0").unwrap();
    assert_eq!(files[0].path, "a.txt");
}

#[test]
fn malformed_numstat_is_an_error() {
    assert!(matches!(
        parse_files("x\t1\ta\0"),
        Err(ParseError::InvalidNumber { .. })
    ));
    assert!(matches!(
        parse_files("1\t1\t\0only-old\0"),
        Err(ParseError::MissingField { .. })
    ));
    assert!(matches!(
        parse_files("garbage\0"),
        Err(ParseError::UnexpectedRecord { .. })
    ));
}

// ---- one file's diff ----

#[test]
fn diff_args_cover_both_sides_of_a_rename_literally() {
    let renamed = FileChange {
        path: "new*.txt".into(),
        old_path: Some("old.txt".into()),
        status: Change::Renamed,
        stat: Numstat::Binary,
    };
    assert_eq!(
        diff_args(&oid(A), &renamed),
        [
            "show",
            "--diff-merges=first-parent",
            "--find-renames",
            "--format=",
            "--no-ext-diff",
            A,
            "--",
            ":(literal)old.txt",
            ":(literal)new*.txt"
        ]
    );
}

#[test]
fn parses_a_unified_diff_into_kinds_and_line_numbers() {
    let out = "diff --git a/f.txt b/f.txt\n\
               index 1111111..2222222 100644\n\
               --- a/f.txt\n\
               +++ b/f.txt\n\
               @@ -1,3 +1,3 @@ fn main\n \
               keep\n\
               -old\n\
               +new\n \
               tail\n\
               \\ No newline at end of file\n";
    let diff = parse_diff(out);
    let kinds: Vec<LineKind> = diff.lines.iter().map(|l| l.kind).collect();
    assert_eq!(
        kinds,
        [
            LineKind::Header,
            LineKind::Header,
            LineKind::Header,
            LineKind::Header,
            LineKind::Hunk,
            LineKind::Context,
            LineKind::Deleted,
            LineKind::Added,
            LineKind::Context,
            LineKind::NoNewline,
        ]
    );
    let numbers: Vec<(Option<u32>, Option<u32>)> = diff
        .lines
        .iter()
        .map(|l| (l.old_line, l.new_line))
        .collect();
    assert_eq!(
        &numbers[5..9],
        [
            (Some(1), Some(1)),
            (Some(2), None),
            (None, Some(2)),
            (Some(3), Some(3))
        ]
    );
    assert_eq!(diff.lines[7].text, "+new");
    assert_eq!((diff.added(), diff.deleted()), (1, 1));
}

#[test]
fn hunk_headers_without_counts_and_new_files() {
    let out = "--- /dev/null\n+++ b/a\n@@ -0,0 +1 @@\n+1\n@@ -10 +20,2 @@\n x\n";
    let diff = parse_diff(out);
    assert_eq!(diff.lines[3].new_line, Some(1));
    assert_eq!(diff.lines[5].old_line, Some(10));
    assert_eq!(diff.lines[5].new_line, Some(20));
    assert_eq!(
        diff.lines[0].kind,
        LineKind::Header,
        "--- is a header, not a deletion"
    );
}

#[test]
fn binary_diffs_are_a_header_only() {
    let diff = parse_diff("diff --git a/x b/x\nBinary files a/x and b/x differ\n");
    assert!(diff.lines.iter().all(|l| l.kind == LineKind::Header));
    assert!(diff.is_binary());
}

#[test]
fn empty_output_is_an_empty_diff() {
    assert!(parse_diff("").lines.is_empty());
}
