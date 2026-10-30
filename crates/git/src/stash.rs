//! `git stash list`: stash entries, newest first. They are read in the log
//! format, so each stash is also a commit the graph can show.

use ggt_model::{Commit, Stash};

use crate::{ParseError, log};

/// Arguments for `git stash list` (without `-C <repo>`), in [`log::FORMAT`].
#[must_use]
pub fn args() -> Vec<String> {
    vec![
        "stash".into(),
        "list".into(),
        "-z".into(),
        format!("--format={}", log::FORMAT),
    ]
}

/// Arguments for `git stash show --patch <selector>`: a stash's changes
/// against the commit it was made on, as one diff.
#[must_use]
pub fn show_args(selector: &str) -> Vec<String> {
    vec![
        "stash".into(),
        "show".into(),
        "--patch".into(),
        selector.into(),
    ]
}

/// The stashes in the output of [`args`]. Position in the output is the
/// stash index (`stash@{0}` first).
pub fn parse(output: &str) -> Result<Vec<Stash>, ParseError> {
    Ok(parse_commits(output)?
        .into_iter()
        .enumerate()
        .map(|(index, commit)| Stash {
            id: commit.id,
            index,
            subject: commit.subject,
        })
        .collect())
}

/// The stashes in the output of [`args`] as commits, in stash order, each
/// with only its first parent: the commit it was made on. Its other
/// parents (the index and untracked files) are git's bookkeeping.
pub fn parse_commits(output: &str) -> Result<Vec<Commit>, ParseError> {
    let mut commits = log::parse(output)?;
    for commit in &mut commits {
        commit.parents.truncate(1);
    }
    Ok(commits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const C: &str = "cccccccccccccccccccccccccccccccccccccccc";
    const D: &str = "dddddddddddddddddddddddddddddddddddddddd";

    #[test]
    fn args_use_nul_separated_format() {
        let args = args();
        assert_eq!(args[..3], ["stash", "list", "-z"]);
    }

    /// A stash record in the log format: id, parents (base, index commit,
    /// untracked commit), author, committer, subject.
    fn record(id: &str, parents: &str, subject: &str) -> String {
        format!("{id}\x1f{parents}\x1fA\x1fa@x\x1f100\x1fC\x1fc@x\x1f200\x1f{subject}\0")
    }

    #[test]
    fn parses_stash_entries_in_order() {
        let out = record(A, &format!("{C} {D}"), "On main: WIP") + &record(B, C, "On dev: older");
        let stashes = parse(&out).unwrap();
        assert_eq!(stashes.len(), 2);
        assert_eq!(stashes[0].index, 0);
        assert_eq!(stashes[0].selector(), "stash@{0}");
        assert_eq!(stashes[0].subject, "On main: WIP");
        assert_eq!(stashes[1].id.as_str(), B);
        assert_eq!(stashes[1].index, 1);
    }

    #[test]
    fn each_stash_is_also_a_commit_on_the_commit_it_was_made_on() {
        let out = record(A, &format!("{C} {D}"), "On main: WIP");
        let commits = parse_commits(&out).unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].id.as_str(), A);
        // Only the base: the index and untracked commits are git's own.
        assert_eq!(commits[0].parents.len(), 1);
        assert_eq!(commits[0].parents[0].as_str(), C);
        assert_eq!(commits[0].subject, "On main: WIP");
        assert_eq!(commits[0].author.name, "A");
    }

    #[test]
    fn empty_output_means_no_stashes() {
        assert_eq!(parse_commits("").unwrap(), vec![]);
        assert_eq!(parse("").unwrap(), vec![]);
    }
}
