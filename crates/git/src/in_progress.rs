//! `git rev-parse --git-path`: which operation (merge, rebase, cherry-pick,
//! revert) stopped halfway and waits to be continued or aborted.
//!
//! Git marks each one with a file or directory in the git directory. `git
//! rev-parse --git-path` names where those live (worktrees keep their own),
//! and [`parse`] checks which of them exist.

use std::path::Path;

/// An operation git stopped in the middle of, usually on a conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Merge,
    Rebase,
    CherryPick,
    Revert,
}

impl Operation {
    /// How the banner names it: `Merge`, `Cherry-pick`.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Merge => "Merge",
            Self::Rebase => "Rebase",
            Self::CherryPick => "Cherry-pick",
            Self::Revert => "Revert",
        }
    }

    /// The git subcommand that continues or aborts it (`merge --continue`).
    #[must_use]
    pub fn subcommand(self) -> &'static str {
        match self {
            Self::Merge => "merge",
            Self::Rebase => "rebase",
            Self::CherryPick => "cherry-pick",
            Self::Revert => "revert",
        }
    }
}

/// Marker paths, in the order [`args`] asks for them and [`parse`] reads
/// them. A rebase comes first: it can leave other markers behind while it runs.
const MARKERS: [(&str, Operation); 5] = [
    ("rebase-merge", Operation::Rebase),
    ("rebase-apply", Operation::Rebase),
    ("CHERRY_PICK_HEAD", Operation::CherryPick),
    ("REVERT_HEAD", Operation::Revert),
    ("MERGE_HEAD", Operation::Merge),
];

/// Arguments for `git rev-parse --git-path <marker>...` (without `-C <repo>`).
#[must_use]
pub fn args() -> Vec<String> {
    let mut args = vec!["rev-parse".to_owned()];
    for (marker, _) in MARKERS {
        args.extend(["--git-path".to_owned(), marker.to_owned()]);
    }
    args
}

/// The operation in progress, from the output of [`args`]: one path per
/// line, relative to the directory git ran in. `exists` checks a path (the
/// app joins it to the repository directory), so this stays testable.
pub fn parse(output: &str, exists: impl Fn(&Path) -> bool) -> Option<Operation> {
    output
        .lines()
        .zip(MARKERS)
        .find(|(path, _)| !path.is_empty() && exists(Path::new(path)))
        .map(|(_, (_, operation))| operation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    const OUTPUT: &str = ".git/rebase-merge\n.git/rebase-apply\n.git/CHERRY_PICK_HEAD\n.git/REVERT_HEAD\n.git/MERGE_HEAD\n";

    fn with(present: &[&str]) -> Option<Operation> {
        parse(OUTPUT, |path| {
            present.iter().any(|p| path == Path::new(".git").join(p))
        })
    }

    #[test]
    fn args_ask_for_every_marker() {
        assert_eq!(
            args().join(" "),
            "rev-parse --git-path rebase-merge --git-path rebase-apply --git-path CHERRY_PICK_HEAD --git-path REVERT_HEAD --git-path MERGE_HEAD"
        );
    }

    #[test]
    fn nothing_in_progress_without_markers() {
        assert_eq!(with(&[]), None);
    }

    #[test]
    fn merge_head_means_a_merge() {
        assert_eq!(with(&["MERGE_HEAD"]), Some(Operation::Merge));
    }

    #[test]
    fn either_rebase_directory_means_a_rebase() {
        assert_eq!(with(&["rebase-merge"]), Some(Operation::Rebase));
        assert_eq!(with(&["rebase-apply"]), Some(Operation::Rebase));
    }

    #[test]
    fn cherry_pick_head_means_a_cherry_pick() {
        assert_eq!(with(&["CHERRY_PICK_HEAD"]), Some(Operation::CherryPick));
    }

    #[test]
    fn revert_head_means_a_revert() {
        assert_eq!(with(&["REVERT_HEAD"]), Some(Operation::Revert));
    }

    #[test]
    fn a_rebase_wins_over_the_markers_it_leaves_behind() {
        assert_eq!(
            with(&["MERGE_HEAD", "CHERRY_PICK_HEAD", "rebase-merge"]),
            Some(Operation::Rebase)
        );
    }

    #[test]
    fn paths_in_a_linked_worktree_are_checked_as_printed() {
        let output = "/r/.git/worktrees/w/rebase-merge\n/r/.git/worktrees/w/rebase-apply\n/r/.git/worktrees/w/CHERRY_PICK_HEAD\n/r/.git/worktrees/w/REVERT_HEAD\n/r/.git/worktrees/w/MERGE_HEAD\n";
        let found = parse(output, |p| p == Path::new("/r/.git/worktrees/w/MERGE_HEAD"));
        assert_eq!(found, Some(Operation::Merge));
    }

    #[test]
    fn labels_and_subcommands() {
        let all = [
            Operation::Merge,
            Operation::Rebase,
            Operation::CherryPick,
            Operation::Revert,
        ];
        let labels: Vec<&str> = all.iter().map(|o| o.label()).collect();
        let commands: Vec<&str> = all.iter().map(|o| o.subcommand()).collect();
        assert_eq!(labels, ["Merge", "Rebase", "Cherry-pick", "Revert"]);
        assert_eq!(commands, ["merge", "rebase", "cherry-pick", "revert"]);
    }
}
