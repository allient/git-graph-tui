//! `git worktree list`: the repository's working trees, for the worktrees
//! view (S20 4).

use tgg_model::Oid;

use crate::ParseError;
use crate::error::oid;

/// One working tree of the repository.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Worktree {
    /// Its folder, as git prints it (absolute).
    pub path: String,
    /// The commit checked out there; `None` for a bare repository.
    pub head: Option<Oid>,
    /// The branch checked out there, without `refs/heads/`; `None` when
    /// HEAD is detached or the repository is bare.
    pub branch: Option<String>,
    pub detached: bool,
    /// The main worktree of a bare repository (no files checked out).
    pub bare: bool,
    /// `Some(reason)` when locked; the reason may be empty.
    pub locked: Option<String>,
    /// `Some(reason)` when git would prune it (its folder is gone).
    pub prunable: Option<String>,
    /// The main worktree, which git lists first and can't remove.
    pub is_main: bool,
}

/// Arguments for `git worktree list` (without `-C <repo>`).
#[must_use]
pub fn args() -> Vec<String> {
    ["worktree", "list", "--porcelain", "-z"]
        .map(str::to_owned)
        .to_vec()
}

/// Parses `git worktree list --porcelain -z`: NUL-terminated `key value`
/// lines, with an empty line after each worktree. Lines this parser doesn't
/// know are skipped, so newer git versions keep working.
pub fn parse(output: &str) -> Result<Vec<Worktree>, ParseError> {
    let mut worktrees: Vec<Worktree> = Vec::new();
    let mut current: Option<Worktree> = None;
    for line in output.split('\0') {
        if line.is_empty() {
            worktrees.extend(current.take());
            continue;
        }
        let record = worktrees.len();
        let (key, value) = line.split_once(' ').unwrap_or((line, ""));
        if key == "worktree" {
            worktrees.extend(current.take());
            current = Some(Worktree {
                path: value.to_owned(),
                is_main: record == 0,
                ..Worktree::default()
            });
            continue;
        }
        let Some(worktree) = &mut current else {
            return Err(ParseError::UnexpectedRecord {
                record,
                text: line.to_owned(),
            });
        };
        match key {
            "HEAD" => worktree.head = Some(oid(value, record, "HEAD")?),
            "branch" => {
                let name = value.strip_prefix("refs/heads/").unwrap_or(value);
                worktree.branch = Some(name.to_owned());
            }
            "detached" => worktree.detached = true,
            "bare" => worktree.bare = true,
            "locked" => worktree.locked = Some(value.to_owned()),
            "prunable" => worktree.prunable = Some(value.to_owned()),
            _ => {}
        }
    }
    worktrees.extend(current);
    Ok(worktrees)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    #[test]
    fn args_ask_for_nul_separated_porcelain() {
        assert_eq!(args(), ["worktree", "list", "--porcelain", "-z"]);
    }

    #[test]
    fn parses_every_kind_of_worktree() {
        let out = format!(
            "worktree /r/main\0bare\0\0\
             worktree /r/feat\0HEAD {A}\0branch refs/heads/feature/x\0\0\
             worktree /r/usb\0HEAD {A}\0detached\0locked on a usb stick\0\0\
             worktree /r/gone\0HEAD {A}\0branch refs/heads/old\0locked\0prunable gitdir file points to non-existent location\0\0"
        );
        let head = Some(Oid::parse(A).unwrap());
        assert_eq!(
            parse(&out).unwrap(),
            [
                Worktree {
                    path: "/r/main".into(),
                    bare: true,
                    is_main: true,
                    ..Worktree::default()
                },
                Worktree {
                    path: "/r/feat".into(),
                    head: head.clone(),
                    branch: Some("feature/x".into()),
                    ..Worktree::default()
                },
                Worktree {
                    path: "/r/usb".into(),
                    head: head.clone(),
                    detached: true,
                    locked: Some("on a usb stick".into()),
                    ..Worktree::default()
                },
                Worktree {
                    path: "/r/gone".into(),
                    head,
                    branch: Some("old".into()),
                    locked: Some(String::new()),
                    prunable: Some("gitdir file points to non-existent location".into()),
                    ..Worktree::default()
                },
            ]
        );
    }

    #[test]
    fn paths_keep_spaces_and_unknown_lines_are_skipped() {
        let out = format!("worktree /r/my repo\0HEAD {A}\0branch refs/heads/main\0future x\0\0");
        let worktrees = parse(&out).unwrap();
        assert_eq!(worktrees[0].path, "/r/my repo");
        assert_eq!(worktrees[0].branch.as_deref(), Some("main"));
    }

    #[test]
    fn a_bad_head_is_an_error() {
        let out = "worktree /r\0HEAD nope\0\0";
        assert!(matches!(
            parse(out),
            Err(ParseError::InvalidOid {
                record: 0,
                field: "HEAD",
                ..
            })
        ));
    }

    #[test]
    fn a_line_before_any_worktree_is_an_error() {
        assert_eq!(
            parse("HEAD x\0\0"),
            Err(ParseError::UnexpectedRecord {
                record: 0,
                text: "HEAD x".into()
            })
        );
    }

    #[test]
    fn empty_output_means_no_worktrees() {
        assert_eq!(parse("").unwrap(), vec![]);
    }
}
