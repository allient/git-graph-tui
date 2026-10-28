//! `git log`: commits for the graph, one page at a time.

use tgg_model::{Commit, Signature};

use crate::ParseError;
use crate::error::{fields, oid};

/// id, parents, author name/email/time, committer name/email/time, subject.
pub const FORMAT: &str = "%H%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%cn%x1f%ce%x1f%ct%x1f%s";
const FIELDS: [&str; 9] = [
    "id",
    "parents",
    "author name",
    "author email",
    "author time",
    "committer name",
    "committer email",
    "committer time",
    "subject",
];

/// Which page of history to load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogQuery {
    pub max_count: usize,
    pub skip: usize,
    pub revisions: Revisions,
}

/// Whose history to load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Revisions {
    /// Every local branch, tag and HEAD; `remotes` adds remote-tracking branches.
    All { remotes: bool },
    /// Only the history of these refs (full names like `refs/heads/main`).
    Only(Vec<String>),
}

impl Default for Revisions {
    fn default() -> Self {
        Self::All { remotes: true }
    }
}

/// Arguments for `git log` (without `-C <repo>`).
#[must_use]
pub fn args(query: &LogQuery) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "log".into(),
        "--topo-order".into(),
        "-z".into(),
        format!("--format={FORMAT}"),
        format!("--max-count={}", query.max_count),
        format!("--skip={}", query.skip),
    ];
    match &query.revisions {
        Revisions::All { remotes } => {
            args.push("--branches".into());
            if *remotes {
                args.push("--remotes".into());
            }
            args.extend(["--tags".into(), "HEAD".into()]);
        }
        // `--end-of-options` keeps a ref named like an option from being parsed as one.
        Revisions::Only(refs) => {
            args.push("--end-of-options".into());
            args.extend(refs.iter().cloned());
        }
    }
    args.push("--".into());
    args
}

/// Parses NUL-separated records produced with [`FORMAT`].
pub fn parse(output: &str) -> Result<Vec<Commit>, ParseError> {
    output
        .split('\0')
        .map(|record| record.trim_start_matches('\n'))
        .filter(|record| !record.is_empty())
        .enumerate()
        .map(|(index, record)| parse_record(record, index))
        .collect()
}

fn parse_record(record: &str, index: usize) -> Result<Commit, ParseError> {
    let f = fields(record, index, &FIELDS)?;
    let parents = f[1]
        .split_whitespace()
        .map(|p| oid(p, index, "parents"))
        .collect::<Result<_, _>>()?;
    Ok(Commit {
        id: oid(f[0], index, "id")?,
        parents,
        author: Signature {
            name: f[2].to_owned(),
            email: f[3].to_owned(),
            time: number(f[4], index, "author time")?,
        },
        committer: Signature {
            name: f[5].to_owned(),
            email: f[6].to_owned(),
            time: number(f[7], index, "committer time")?,
        },
        subject: f[8].to_owned(),
    })
}

fn number(value: &str, record: usize, field: &'static str) -> Result<i64, ParseError> {
    value.parse().map_err(|_| ParseError::InvalidNumber {
        record,
        field,
        value: value.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const C: &str = "cccccccccccccccccccccccccccccccccccccccc";

    fn record(id: &str, parents: &str, subject: &str) -> String {
        format!(
            "{id}\x1f{parents}\x1fAnn\x1fann@x.io\x1f100\x1fCom\x1fcom@x.io\x1f200\x1f{subject}"
        )
    }

    #[test]
    fn args_request_nul_separated_machine_format() {
        let args = args(&LogQuery {
            max_count: 50,
            skip: 100,
            revisions: Revisions::default(),
        });
        assert_eq!(args[0], "log");
        assert!(args.contains(&"-z".to_owned()));
        assert!(args.contains(&"--max-count=50".to_owned()));
        assert!(args.contains(&"--skip=100".to_owned()));
        assert!(args.contains(&format!("--format={FORMAT}")));
    }

    #[test]
    fn parses_commits_with_parents_and_signatures() {
        let out = format!(
            "{}\0{}\0",
            record(A, &format!("{B} {C}"), "Merge"),
            record(B, "", "Root")
        );
        let commits = parse(&out).unwrap();
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].id.as_str(), A);
        assert_eq!(commits[0].parents.len(), 2);
        assert_eq!(commits[0].author.name, "Ann");
        assert_eq!(commits[0].author.email, "ann@x.io");
        assert_eq!(commits[0].author.time, 100);
        assert_eq!(commits[0].committer.name, "Com");
        assert_eq!(commits[0].committer.time, 200);
        assert_eq!(commits[0].subject, "Merge");
        assert!(commits[1].is_root());
    }

    #[test]
    fn keeps_unusual_characters_in_subjects() {
        let subject = "fix: tabs\tpipes | quotes \" and émoji 🚀";
        let commits = parse(&format!("{}\0", record(A, "", subject))).unwrap();
        assert_eq!(commits[0].subject, subject);
    }

    #[test]
    fn empty_output_means_no_commits() {
        assert_eq!(parse("").unwrap(), vec![]);
    }

    #[test]
    fn tolerates_newlines_between_records() {
        // `git log -z` separates records with NUL; some versions add a newline after it.
        let out = format!("{}\0\n{}\0", record(A, "", "a"), record(B, "", "b"));
        assert_eq!(parse(&out).unwrap().len(), 2);
    }

    #[test]
    fn reports_truncated_records() {
        let err = parse(&format!("{A}\x1f\x1fAnn\0")).unwrap_err();
        assert!(matches!(err, ParseError::MissingField { .. }), "{err:?}");
    }

    #[test]
    fn reports_invalid_ids_and_times() {
        assert!(matches!(
            parse(&format!("{}\0", record("xyz", "", "s"))),
            Err(ParseError::InvalidOid { .. })
        ));
        let bad_time = record(A, "", "s").replace("\x1f100\x1f", "\x1fsoon\x1f");
        assert!(matches!(
            parse(&format!("{bad_time}\0")),
            Err(ParseError::InvalidNumber { .. })
        ));
    }
}
