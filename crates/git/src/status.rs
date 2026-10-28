//! `git status --porcelain=v2`: HEAD state and uncommitted changes.

use tgg_model::{HeadState, WorkingTreeStatus};

use crate::ParseError;

/// Arguments for `git status` (without `-C <repo>`).
#[must_use]
pub fn args() -> Vec<String> {
    [
        "status",
        "--porcelain=v2",
        "-z",
        "--branch",
        "--untracked-files=all",
    ]
    .map(String::from)
    .to_vec()
}

/// Parses `--porcelain=v2 -z --branch` output.
pub fn parse(output: &str) -> Result<WorkingTreeStatus, ParseError> {
    let mut status = WorkingTreeStatus::default();
    let mut unborn = false;
    let mut records = output.split('\0').filter(|r| !r.is_empty()).enumerate();

    while let Some((index, record)) = records.next() {
        if let Some(header) = record.strip_prefix("# ") {
            if header == "branch.oid (initial)" {
                unborn = true;
            } else if let Some(head) = header.strip_prefix("branch.head ") {
                status.head = if head == "(detached)" {
                    HeadState::Detached
                } else {
                    HeadState::Branch(head.to_owned())
                };
            }
            continue;
        }
        match record.as_bytes().first() {
            Some(b'1') => count_xy(&mut status, record, index)?,
            Some(b'2') => {
                count_xy(&mut status, record, index)?;
                // Renames and copies carry their original path as the next record.
                records.next();
            }
            Some(b'u') => {
                status.files += 1;
                status.conflicted += 1;
            }
            Some(b'?') => {
                status.files += 1;
                status.untracked += 1;
            }
            Some(b'!') => {}
            _ => {
                return Err(ParseError::UnexpectedRecord {
                    record: index,
                    text: record.to_owned(),
                });
            }
        }
    }

    if unborn {
        status.head = HeadState::Unborn;
    }
    Ok(status)
}

/// Counts an ordinary or renamed entry from its `XY` field (`.` means unchanged).
fn count_xy(status: &mut WorkingTreeStatus, record: &str, index: usize) -> Result<(), ParseError> {
    let xy = record
        .get(2..4)
        .ok_or(ParseError::MissingField {
            record: index,
            field: "XY",
        })?
        .as_bytes();
    status.files += 1;
    if xy[0] != b'.' {
        status.staged += 1;
    }
    if xy[1] != b'.' {
        status.unstaged += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tgg_model::HeadState;

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn ordinary(xy: &str, path: &str) -> String {
        format!("1 {xy} N... 100644 100644 100644 {A} {A} {path}")
    }

    #[test]
    fn args_use_porcelain_v2_with_nul_separators() {
        let args = args();
        assert_eq!(args[..4], ["status", "--porcelain=v2", "-z", "--branch"]);
    }

    #[test]
    fn clean_tree_on_a_branch() {
        let out = format!("# branch.oid {A}\0# branch.head main\0");
        let status = parse(&out).unwrap();
        assert_eq!(status.head, HeadState::Branch("main".into()));
        assert!(status.is_clean());
    }

    #[test]
    fn detached_and_unborn_heads() {
        let detached = parse(&format!("# branch.oid {A}\0# branch.head (detached)\0")).unwrap();
        assert_eq!(detached.head, HeadState::Detached);
        let unborn = parse("# branch.oid (initial)\0# branch.head main\0").unwrap();
        assert_eq!(unborn.head, HeadState::Unborn);
    }

    #[test]
    fn counts_staged_unstaged_untracked_and_conflicted_files() {
        let out = [
            "# branch.oid (initial)".to_owned(),
            "# branch.head main".to_owned(),
            ordinary("M.", "staged.rs"),
            ordinary(".M", "unstaged.rs"),
            ordinary("MM", "both.rs"),
            format!("u UU N... 100644 100644 100644 100644 {A} {A} {A} conflict.rs"),
            "? new file.txt".to_owned(),
            "! ignored.log".to_owned(),
        ]
        .join("\0");
        let status = parse(&out).unwrap();
        assert_eq!(status.files, 5, "ignored files don't count");
        assert_eq!(status.staged, 2);
        assert_eq!(status.unstaged, 2);
        assert_eq!(status.conflicted, 1);
        assert_eq!(status.untracked, 1);
    }

    #[test]
    fn renames_consume_their_original_path() {
        // A rename entry is followed by an extra NUL-separated original path.
        let out = format!(
            "# branch.oid {A}\0# branch.head main\0\
             2 R. N... 100644 100644 100644 {A} {A} R100 new.rs\0old.rs\0? x\0"
        );
        let status = parse(&out).unwrap();
        assert_eq!(status.files, 2);
        assert_eq!(status.staged, 1);
        assert_eq!(status.untracked, 1);
    }

    #[test]
    fn rejects_unknown_entries() {
        assert!(matches!(
            parse("Z nonsense\0"),
            Err(ParseError::UnexpectedRecord { .. })
        ));
    }
}
