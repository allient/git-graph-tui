//! `git show`: one commit's details, its changed files and a file's diff (S10).
//!
//! Merge commits are always compared with their first parent
//! (`--diff-merges=first-parent`), the branch the merge was made on, so a
//! merge lists what it brought in. Root commits need nothing special:
//! `git show` diffs them against the empty tree, which also keeps this
//! independent of the object format (SHA-1 or SHA-256 empty-tree ids).

use tgg_model::{Commit, Oid, Signature};

use crate::ParseError;
use crate::error::{fields, oid};

/// id, parents, author name/email/time, committer name/email/time, raw message.
pub const DETAILS_FORMAT: &str = "%H%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%cn%x1f%ce%x1f%ct%x1f%B";
const DETAILS_FIELDS: [&str; 9] = [
    "id",
    "parents",
    "author name",
    "author email",
    "author time",
    "committer name",
    "committer email",
    "committer time",
    "message",
];

/// A commit with its whole message, for the details panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitDetails {
    /// `subject` is the first line of `message`.
    pub commit: Commit,
    /// The full message as written (subject, blank line, body).
    pub message: String,
}

impl CommitDetails {
    /// The message without its subject line, trimmed.
    #[must_use]
    pub fn body(&self) -> &str {
        self.message
            .split_once('\n')
            .map_or("", |(_, rest)| rest)
            .trim_matches('\n')
    }
}

/// Arguments for the details of `commit` (without `-C <repo>`).
#[must_use]
pub fn details_args(commit: &Oid) -> Vec<String> {
    vec![
        "show".into(),
        "--no-patch".into(),
        "-z".into(),
        format!("--format={DETAILS_FORMAT}"),
        commit.to_string(),
        "--".into(),
    ]
}

/// Parses the output of [`details_args`].
pub fn parse_details(output: &str) -> Result<CommitDetails, ParseError> {
    let record = output
        .trim_start_matches('\n')
        .split('\0')
        .next()
        .unwrap_or_default();
    let f = fields(record, 0, &DETAILS_FIELDS)?;
    let parents = f[1]
        .split_whitespace()
        .map(|p| oid(p, 0, "parents"))
        .collect::<Result<_, _>>()?;
    let message = f[8].trim_end_matches('\n').to_owned();
    let subject = message.lines().next().unwrap_or_default().to_owned();
    Ok(CommitDetails {
        commit: Commit {
            id: oid(f[0], 0, "id")?,
            parents,
            author: Signature {
                name: f[2].to_owned(),
                email: f[3].to_owned(),
                time: number(f[4], 0, "author time")?,
            },
            committer: Signature {
                name: f[5].to_owned(),
                email: f[6].to_owned(),
                time: number(f[7], 0, "committer time")?,
            },
            subject,
        },
        message,
    })
}

/// Lines added and deleted in one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Numstat {
    Lines {
        added: u32,
        deleted: u32,
    },
    /// git prints `-` for both counts.
    Binary,
}

/// A file a commit changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    /// Path after the commit.
    pub path: String,
    /// Path before the commit, for renames and copies.
    pub old_path: Option<String>,
    pub stat: Numstat,
}

/// Arguments for the files `commit` changed, with line counts.
#[must_use]
pub fn files_args(commit: &Oid) -> Vec<String> {
    vec![
        "show".into(),
        "--diff-merges=first-parent".into(),
        "--find-renames".into(),
        "--numstat".into(),
        "-z".into(),
        // No header: only the numstat records.
        "--format=".into(),
        commit.to_string(),
        "--".into(),
    ]
}

/// Parses `--numstat -z` output.
///
/// Each record is `added\tdeleted\tpath\0`. For a rename the path is empty
/// and the old and new paths follow as two more NUL-terminated fields.
/// Paths can contain tabs, so only the first two tabs separate counts.
pub fn parse_files(output: &str) -> Result<Vec<FileChange>, ParseError> {
    let mut files = Vec::new();
    let mut tokens = output.split('\0');
    while let Some(token) = tokens.next() {
        let token = token.trim_start_matches('\n');
        if token.is_empty() {
            continue;
        }
        let record = files.len();
        let mut parts = token.splitn(3, '\t');
        let (Some(added), Some(deleted), Some(path)) = (parts.next(), parts.next(), parts.next())
        else {
            return Err(ParseError::UnexpectedRecord {
                record,
                text: token.to_owned(),
            });
        };
        let stat = if added == "-" || deleted == "-" {
            Numstat::Binary
        } else {
            Numstat::Lines {
                added: count(added, record, "added")?,
                deleted: count(deleted, record, "deleted")?,
            }
        };
        let (path, old_path) = if path.is_empty() {
            let mut next = |field| {
                tokens
                    .next()
                    .filter(|p| !p.is_empty())
                    .map(str::to_owned)
                    .ok_or(ParseError::MissingField { record, field })
            };
            let old = next("old path")?;
            (next("new path")?, Some(old))
        } else {
            (path.to_owned(), None)
        };
        files.push(FileChange {
            path,
            old_path,
            stat,
        });
    }
    Ok(files)
}

/// Arguments for the unified diff of one file in `commit`, using both
/// sides of a rename. Paths are literal pathspecs, so names with `*` or `:`
/// are never treated as patterns.
#[must_use]
pub fn diff_args(commit: &Oid, file: &FileChange) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "show".into(),
        "--diff-merges=first-parent".into(),
        "--find-renames".into(),
        "--format=".into(),
        // A configured external diff tool would print something else.
        "--no-ext-diff".into(),
        commit.to_string(),
        "--".into(),
    ];
    args.extend(
        file.old_path
            .iter()
            .chain(std::iter::once(&file.path))
            .map(|p| format!(":(literal){p}")),
    );
    args
}

/// Arguments for the whole content of `path` as it was in `commit`
/// (`git show <commit>:<path>`). `path` is relative to the repository root;
/// git fails when the file doesn't exist in that commit (it deleted it).
#[must_use]
pub fn file_at_args(commit: &Oid, path: &str) -> Vec<String> {
    vec!["show".into(), format!("{commit}:{path}")]
}

/// What a diff line is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// `diff --git`, `index`, `---`/`+++`, mode, rename and `Binary files` lines.
    Header,
    /// `@@ -a,b +c,d @@`
    Hunk,
    Added,
    Deleted,
    Context,
    /// `\ No newline at end of file`
    NoNewline,
}

/// One line of a unified diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: LineKind,
    /// The line as git printed it, including its `+`/`-`/` ` prefix.
    pub text: String,
    /// Line number in the old file (context and deleted lines).
    pub old_line: Option<u32>,
    /// Line number in the new file (context and added lines).
    pub new_line: Option<u32>,
}

/// A parsed unified diff.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Diff {
    pub lines: Vec<DiffLine>,
}

impl Diff {
    #[must_use]
    pub fn added(&self) -> usize {
        self.count(LineKind::Added)
    }

    #[must_use]
    pub fn deleted(&self) -> usize {
        self.count(LineKind::Deleted)
    }

    /// git reported the file as binary (no hunks to show).
    #[must_use]
    pub fn is_binary(&self) -> bool {
        self.lines
            .iter()
            .any(|l| l.kind == LineKind::Header && l.text.starts_with("Binary files "))
    }

    fn count(&self, kind: LineKind) -> usize {
        self.lines.iter().filter(|l| l.kind == kind).count()
    }
}

/// Parses `git show` patch output. Never fails: unknown lines are headers.
#[must_use]
pub fn parse_diff(output: &str) -> Diff {
    let mut lines = Vec::new();
    let mut in_hunk = false;
    let (mut old, mut new) = (0u32, 0u32);
    for text in output.lines() {
        let (kind, old_line, new_line) = if text.starts_with("@@") {
            (old, new) = hunk_starts(text);
            in_hunk = true;
            (LineKind::Hunk, None, None)
        } else if !in_hunk || text.starts_with("diff --git ") {
            in_hunk = false;
            (LineKind::Header, None, None)
        } else if text.starts_with('+') {
            new += 1;
            (LineKind::Added, None, Some(new - 1))
        } else if text.starts_with('-') {
            old += 1;
            (LineKind::Deleted, Some(old - 1), None)
        } else if text.starts_with('\\') {
            (LineKind::NoNewline, None, None)
        } else {
            old += 1;
            new += 1;
            (LineKind::Context, Some(old - 1), Some(new - 1))
        };
        lines.push(DiffLine {
            kind,
            text: text.to_owned(),
            old_line,
            new_line,
        });
    }
    Diff { lines }
}

/// First old and new line numbers of `@@ -a[,b] +c[,d] @@`.
fn hunk_starts(header: &str) -> (u32, u32) {
    let start = |sign: char| {
        header
            .split_whitespace()
            .find_map(|part| part.strip_prefix(sign))
            .and_then(|range| range.split(',').next())
            .and_then(|n| n.parse().ok())
            .unwrap_or(0)
    };
    (start('-'), start('+'))
}

fn number(value: &str, record: usize, field: &'static str) -> Result<i64, ParseError> {
    value.parse().map_err(|_| ParseError::InvalidNumber {
        record,
        field,
        value: value.to_owned(),
    })
}

fn count(value: &str, record: usize, field: &'static str) -> Result<u32, ParseError> {
    value.parse().map_err(|_| ParseError::InvalidNumber {
        record,
        field,
        value: value.to_owned(),
    })
}

#[cfg(test)]
mod tests;
