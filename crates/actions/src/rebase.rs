//! Interactive rebase (S20 4): the commits after a base, oldest first like
//! git's todo list, each with what to do with it (pick, squash, fixup,
//! drop), and the plan that runs `git rebase --interactive` with that list.

use std::path::Path;

use ggt_git::GitCommand;
use ggt_model::Oid;

use crate::action::{Confirmation, Plan};

/// What happens to one commit, as git's todo list names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TodoAction {
    /// Keep the commit.
    Pick,
    /// Meld it into the commit before it, keeping both messages.
    Squash,
    /// Meld it into the commit before it, keeping only that one's message.
    Fixup,
    /// Remove the commit and its changes.
    Drop,
}

impl TodoAction {
    /// In the order Space cycles through them.
    pub const ALL: [Self; 4] = [Self::Pick, Self::Squash, Self::Fixup, Self::Drop];

    /// The todo list's word for it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Pick => "pick",
            Self::Squash => "squash",
            Self::Fixup => "fixup",
            Self::Drop => "drop",
        }
    }

    /// Melds into the commit before it.
    #[must_use]
    pub fn melds(self) -> bool {
        matches!(self, Self::Squash | Self::Fixup)
    }
}

/// One line of the todo list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TodoItem {
    pub id: Oid,
    pub subject: String,
    pub action: TodoAction,
}

/// Which way to move a commit in the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Earlier (towards the base).
    Up,
    /// Later (towards HEAD).
    Down,
}

/// Why the list can't change that way, or can't run yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RebaseError {
    #[error("There is no kept commit before this one to squash it into.")]
    NothingToSquashInto,
    #[error("Keep at least one commit: to drop them all, reset the branch instead.")]
    NoneKept,
    #[error("Nothing to change: every commit is picked, in its original order.")]
    Unchanged,
}

/// An interactive rebase of `branch` onto `base`, being edited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractiveRebase {
    /// The commit the edited ones go on top of (unchanged itself).
    pub base: Oid,
    /// The branch HEAD is on, which the rebase rewrites.
    pub branch: String,
    /// Oldest first, as git applies them.
    pub items: Vec<TodoItem>,
    /// The commits in their original order, to spot a list with no change.
    original: Vec<Oid>,
}

impl InteractiveRebase {
    /// Every commit picked, in the given order (oldest first).
    #[must_use]
    pub fn new(base: Oid, branch: String, commits: Vec<(Oid, String)>) -> Self {
        let original = commits.iter().map(|(id, _)| id.clone()).collect();
        let items = commits
            .into_iter()
            .map(|(id, subject)| TodoItem {
                id,
                subject,
                action: TodoAction::Pick,
            })
            .collect();
        Self {
            base,
            branch,
            items,
            original,
        }
    }

    /// Whether a commit before `index` is kept, so `index` can meld into it.
    fn has_kept_before(&self, index: usize) -> bool {
        self.items[..index.min(self.items.len())]
            .iter()
            .any(|item| item.action != TodoAction::Drop)
    }

    /// Sets what happens to the commit at `index`. Squash and fixup need a
    /// kept commit before it.
    pub fn set_action(&mut self, index: usize, action: TodoAction) -> Result<(), RebaseError> {
        if action.melds() && !self.has_kept_before(index) {
            return Err(RebaseError::NothingToSquashInto);
        }
        if let Some(item) = self.items.get_mut(index) {
            item.action = action;
        }
        Ok(())
    }

    /// The next action for the commit at `index`, skipping squash and
    /// fixup where there is nothing to meld into.
    pub fn cycle(&mut self, index: usize) {
        let Some(current) = self.items.get(index).map(|item| item.action) else {
            return;
        };
        let start = TodoAction::ALL.iter().position(|a| *a == current);
        let next = (1..=TodoAction::ALL.len())
            .map(|step| TodoAction::ALL[(start.unwrap_or(0) + step) % TodoAction::ALL.len()])
            .find(|action| !action.melds() || self.has_kept_before(index));
        if let Some(action) = next {
            self.items[index].action = action;
        }
    }

    /// Moves the commit at `index` one place; returns where it is now.
    pub fn move_item(&mut self, index: usize, direction: Direction) -> usize {
        let to = match direction {
            Direction::Up => index.checked_sub(1),
            Direction::Down => Some(index + 1).filter(|to| *to < self.items.len()),
        };
        match to {
            Some(to) if index < self.items.len() => {
                self.items.swap(index, to);
                to
            }
            _ => index,
        }
    }

    /// Commits left after the rebase: squash and fixup meld, drop removes.
    #[must_use]
    pub fn result_len(&self) -> usize {
        self.items
            .iter()
            .filter(|item| item.action == TodoAction::Pick)
            .count()
    }

    /// `3 commits → 2`.
    #[must_use]
    pub fn preview(&self) -> String {
        format!("{} → {}", commits(self.items.len()), self.result_len())
    }

    /// Whether git can run this list, and whether it changes anything.
    pub fn validate(&self) -> Result<(), RebaseError> {
        if (0..self.items.len()).any(|i| self.items[i].action.melds() && !self.has_kept_before(i)) {
            return Err(RebaseError::NothingToSquashInto);
        }
        if self.result_len() == 0 {
            return Err(RebaseError::NoneKept);
        }
        let reordered = self
            .items
            .iter()
            .map(|item| &item.id)
            .ne(self.original.iter());
        let edited = self
            .items
            .iter()
            .any(|item| item.action != TodoAction::Pick);
        if !reordered && !edited {
            return Err(RebaseError::Unchanged);
        }
        Ok(())
    }

    /// The todo list git runs: `pick <hash> <subject>`, one per line.
    #[must_use]
    pub fn todo(&self) -> String {
        self.lines().map(|line| line + "\n").collect()
    }

    fn lines(&self) -> impl Iterator<Item = String> + '_ {
        self.items.iter().map(|item| {
            // Git reads only the hash; the subject is for people. Subjects
            // are one line, but never let one start another instruction.
            let subject = item.subject.replace(['\n', '\r'], " ");
            format!("{} {} {subject}", item.action.word(), item.id)
        })
    }

    /// `git rebase --interactive <base>` with the todo list replaced by ours
    /// and no editor for the combined squash message (git keeps both
    /// messages). Never autostashes: the app refuses a dirty tree instead.
    /// `missingCommitsCheck=error` makes git stop, rather than silently
    /// drop, a commit our list doesn't name (HEAD moved since the panel
    /// opened): every commit we remove is an explicit `drop`.
    pub(crate) fn plan(&self, repo: &Path) -> Plan {
        let command = GitCommand::write(
            repo,
            [
                "-c".to_owned(),
                "rebase.missingCommitsCheck=error".to_owned(),
                "rebase".to_owned(),
                "--interactive".to_owned(),
                "--no-autostash".to_owned(),
                self.base.to_string(),
            ],
        )
        .without_editor()
        .with_sequence_editor(sequence_editor(self.lines()));
        Plan::Git(vec![command])
    }

    pub(crate) fn confirmation(&self) -> Confirmation {
        Confirmation::new(
            "Interactive Rebase",
            format!(
                "Rewrite {} on '{}'? The old commits stay reachable in the reflog.",
                commits(self.items.len()),
                self.branch
            ),
            "Rebase",
        )
    }
}

/// `1 commit`, `3 commits`.
fn commits(n: usize) -> String {
    if n == 1 {
        "1 commit".to_owned()
    } else {
        format!("{n} commits")
    }
}

/// A shell command that writes `lines` to the file git appends as its last
/// argument (the todo list): `printf '%s\n' 'pick …' 'drop …' > "$@"`. Git
/// runs a sequence editor through the shell, adding ` "$@"` itself. This
/// needs no temporary file, so nothing can be left behind.
fn sequence_editor(lines: impl Iterator<Item = String>) -> String {
    let mut command = r"printf '%s\n'".to_owned();
    for line in lines {
        command.push(' ');
        command.push_str(&shell_quote(&line));
    }
    command.push_str(" >");
    command
}

/// Single-quotes `text` for a POSIX shell: nothing inside is expanded.
fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests;
