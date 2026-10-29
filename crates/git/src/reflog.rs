//! `git reflog`: where HEAD has been, newest first, for the reflog view
//! and the recovery of commits no branch points at any more.

use tgg_model::Oid;

use crate::ParseError;
use crate::error::{fields, oid};

/// Entries loaded when the reflog view opens.
pub const DEFAULT_MAX_COUNT: usize = 500;

/// Commit id, reflog selector, reflog subject and the commit's subject. With `--date=unix` the
/// selector is `HEAD@{<unix time of the entry>}`, which is the only way
/// `--format` prints when an entry was written; the `HEAD@{n}` form is the
/// entry's position, rebuilt by [`parse`].
pub const FORMAT: &str = "%H%x1f%gd%x1f%gs%x1f%s";

/// One reflog entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflogEntry {
    /// The commit HEAD pointed at after this entry.
    pub id: Oid,
    /// Position from the newest entry (`HEAD@{index}`).
    pub index: usize,
    /// What moved HEAD, e.g. `checkout: moving from main to dev`.
    pub subject: String,
    /// The commit's own subject (the first line of its message).
    pub commit_subject: String,
    /// When the entry was written, in Unix seconds.
    pub time: i64,
}

impl ReflogEntry {
    /// `HEAD@{index}`.
    #[must_use]
    pub fn selector(&self) -> String {
        format!("HEAD@{{{}}}", self.index)
    }
}

/// Arguments for `git reflog` (without `-C <repo>`): the newest
/// `max_count` entries of HEAD's reflog.
#[must_use]
pub fn args(max_count: usize) -> Vec<String> {
    vec![
        "reflog".into(),
        "show".into(),
        "-z".into(),
        "--date=unix".into(),
        format!("--format={FORMAT}"),
        format!("--max-count={max_count}"),
        "HEAD".into(),
        "--".into(),
    ]
}

/// Parses NUL-separated records produced with [`FORMAT`]. Position in the
/// output is the entry's index (`HEAD@{0}` first).
pub fn parse(output: &str) -> Result<Vec<ReflogEntry>, ParseError> {
    output
        .split('\0')
        .map(|record| record.trim_start_matches('\n'))
        .filter(|record| !record.is_empty())
        .enumerate()
        .map(|(index, record)| {
            let f = fields(
                record,
                index,
                &["id", "selector", "subject", "commit subject"],
            )?;
            Ok(ReflogEntry {
                id: oid(f[0], index, "id")?,
                index,
                subject: f[2].to_owned(),
                commit_subject: f[3].to_owned(),
                time: selector_time(f[1]).ok_or_else(|| ParseError::InvalidNumber {
                    record: index,
                    field: "selector",
                    value: f[1].to_owned(),
                })?,
            })
        })
        .collect()
}

/// The Unix time in a `HEAD@{1700000000}` selector.
fn selector_time(selector: &str) -> Option<i64> {
    let (_, rest) = selector.rsplit_once("@{")?;
    rest.strip_suffix('}')?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    #[test]
    fn args_read_heads_reflog_with_unix_dates() {
        let args = args(500);
        assert_eq!(args[..4], ["reflog", "show", "-z", "--date=unix"]);
        assert!(args.contains(&"--max-count=500".to_owned()), "{args:?}");
        assert_eq!(args[args.len() - 2..], ["HEAD", "--"]);
    }

    #[test]
    fn parses_entries_newest_first() {
        let out = format!(
            "{A}\x1fHEAD@{{1700000120}}\x1freset: moving to HEAD~1\x1fFirst\0\
             {B}\x1fHEAD@{{1700000060}}\x1fcommit: Add login\x1fAdd login\0"
        );
        let entries = parse(&out).unwrap();
        assert_eq!(
            entries,
            [
                ReflogEntry {
                    id: Oid::parse(A).unwrap(),
                    index: 0,
                    subject: "reset: moving to HEAD~1".into(),
                    commit_subject: "First".into(),
                    time: 1_700_000_120,
                },
                ReflogEntry {
                    id: Oid::parse(B).unwrap(),
                    index: 1,
                    subject: "commit: Add login".into(),
                    commit_subject: "Add login".into(),
                    time: 1_700_000_060,
                },
            ]
        );
        assert_eq!(entries[1].selector(), "HEAD@{1}");
    }

    #[test]
    fn subjects_keep_special_characters() {
        let subject = "commit: Fix \"quotes\", tabs\tand ünïcode {braces} @{0}";
        let out = format!("{A}\x1fHEAD@{{5}}\x1f{subject}\x1fs\0{A}\x1fHEAD@{{4}}\x1f\x1fs\0");
        let entries = parse(&out).unwrap();
        assert_eq!(entries[0].subject, subject);
        assert_eq!(entries[1].subject, "", "an entry may have no message");
    }

    #[test]
    fn a_selector_without_a_time_is_an_error() {
        let out = format!("{A}\x1fHEAD@{{now}}\x1fcommit: x\x1fx\0");
        assert_eq!(
            parse(&out),
            Err(ParseError::InvalidNumber {
                record: 0,
                field: "selector",
                value: "HEAD@{now}".into(),
            })
        );
    }

    #[test]
    fn empty_output_means_no_entries() {
        assert_eq!(parse("").unwrap(), vec![]);
    }
}
