use std::path::Path;

use tgg_git::GitCommand;

use crate::action::{Confirmation, Plan, ResetMode, UiEffect, strings};
use crate::input::{self, InputError, InputId, InputKind, InputSpec, InputValue};

/// Reset modes that make sense for uncommitted changes (soft would do nothing).
const RESET_CHOICES: &[&str] = &["mixed", "hard"];

/// What the Source Control menu's commit entries commit (VS Code's
/// Commit, Commit Staged and Commit All).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitScope {
    /// What is staged or, when nothing is, every change.
    Auto,
    Staged,
    All,
}

/// Actions on the "Uncommitted Changes" row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkingTreeAction {
    /// An empty `message` lets git write `WIP on <branch>: ...`.
    Stash {
        message: String,
        include_untracked: bool,
    },
    /// Stashes only what is staged (`git stash push --staged`).
    StashStaged {
        message: String,
    },
    /// Mixed unstages everything; hard discards every change to tracked files.
    Reset {
        mode: ResetMode,
    },
    /// Deletes untracked files and directories (ignored files are kept).
    Clean,
    OpenSourceControl,
    /// Stages files, including deletions (the Source Control pane's `+`).
    Stage {
        paths: Vec<String>,
    },
    /// Unstages files, keeping their changes. Works before the first commit too.
    Unstage {
        paths: Vec<String>,
    },
    /// Commits what is staged or, with `all`, every change (untracked
    /// files too). `amend` rewrites the last commit, keeping its message
    /// when `message` is empty; `signoff` adds a Signed-off-by trailer.
    Commit {
        message: String,
        all: bool,
        amend: bool,
        signoff: bool,
    },
    /// Takes back the last commit, keeping its changes staged.
    UndoLastCommit,
    /// Throws away every unstaged change and deletes untracked files.
    DiscardAll,
    /// Throws away unstaged changes: tracked `paths` go back to their staged
    /// (or committed) content, `untracked` files are deleted.
    Discard {
        paths: Vec<String>,
        untracked: Vec<String>,
    },
}

impl WorkingTreeAction {
    #[must_use]
    pub fn plan(&self, repo: &Path) -> Plan {
        if let Self::Commit { all: true, .. } = self {
            return Plan::Git(vec![
                GitCommand::write(repo, strings(&["add", "--all"])),
                GitCommand::write(repo, self.commit_args()),
            ]);
        }
        if let Self::DiscardAll = self {
            // `:/` is the whole working tree, wherever git runs from.
            return Plan::Git(vec![
                GitCommand::write(repo, strings(&["restore", "--worktree", "--", ":/"])),
                GitCommand::write(repo, strings(&["clean", "-d", "--force", "--", ":/"])),
            ]);
        }
        if let Self::Discard { paths, untracked } = self {
            let mut commands = Vec::new();
            if !paths.is_empty() {
                let args = with_paths(&["restore", "--worktree", "--"], paths);
                commands.push(GitCommand::write(repo, args));
            }
            if !untracked.is_empty() {
                let args = with_paths(&["clean", "--force", "--"], untracked);
                commands.push(GitCommand::write(repo, args));
            }
            return Plan::Git(commands);
        }
        let args = match self {
            Self::Stash {
                message,
                include_untracked,
            } => {
                let mut args = strings(&["stash", "push"]);
                if *include_untracked {
                    args.push("--include-untracked".into());
                }
                if !message.is_empty() {
                    args.extend(strings(&["--message", message]));
                }
                args
            }
            Self::StashStaged { message } => {
                let mut args = strings(&["stash", "push", "--staged"]);
                if !message.is_empty() {
                    args.extend(strings(&["--message", message]));
                }
                args
            }
            // `--` after HEAD: what follows a revision for reset are paths.
            Self::Reset { mode } => {
                strings(&["reset", &format!("--{}", mode.name()), "HEAD", "--"])
            }
            Self::Clean => strings(&["clean", "-d", "--force", "--"]),
            Self::OpenSourceControl => return Plan::Ui(UiEffect::OpenSourceControl),
            Self::Stage { paths } => with_paths(&["add", "--all", "--"], paths),
            Self::Unstage { paths } => with_paths(&["reset", "--quiet", "--"], paths),
            Self::Commit { .. } => self.commit_args(),
            Self::UndoLastCommit => strings(&["reset", "--soft", "HEAD~1", "--"]),
            Self::Discard { .. } | Self::DiscardAll => unreachable!("planned above"),
        };
        Plan::Git(vec![GitCommand::write(repo, args)])
    }

    fn commit_args(&self) -> Vec<String> {
        let Self::Commit {
            message,
            amend,
            signoff,
            ..
        } = self
        else {
            return Vec::new();
        };
        let mut args = strings(&["commit"]);
        if *amend {
            args.push("--amend".into());
        }
        if *signoff {
            args.push("--signoff".into());
        }
        if *amend && message.trim().is_empty() {
            args.push("--no-edit".into());
        } else {
            // One `--message=` argument, so a message starting with `-` stays a message.
            args.push(format!("--message={message}"));
        }
        args
    }

    #[must_use]
    pub fn is_destructive(&self) -> bool {
        matches!(
            self,
            Self::Reset {
                mode: ResetMode::Hard
            } | Self::Clean
                | Self::Discard { .. }
                | Self::DiscardAll
        )
    }

    #[must_use]
    pub fn confirmation(&self) -> Option<Confirmation> {
        if let Self::Discard { paths, untracked } = self {
            let message = match (paths.as_slice(), untracked.as_slice()) {
                ([path], []) => format!("Discard the changes in '{path}'? This can't be undone."),
                ([], [path]) => {
                    format!("Delete the untracked file '{path}'? This can't be undone.")
                }
                _ => format!(
                    "Discard the changes in {} files? Untracked files are deleted. This can't be undone.",
                    paths.len() + untracked.len()
                ),
            };
            return Some(Confirmation::new("Discard Changes", message, "Discard"));
        }
        let (title, message, label) = match self {
            Self::DiscardAll => (
                "Discard All Changes",
                "Discard ALL unstaged changes and delete every untracked file? Staged changes are kept. This can't be undone.",
                "Discard",
            ),
            Self::Reset {
                mode: ResetMode::Hard,
            } => (
                "Reset Uncommitted Changes",
                "Discard ALL uncommitted changes to tracked files? This can't be undone (git reset --hard HEAD). Untracked files are kept.",
                "Discard",
            ),
            Self::Reset { .. } => (
                "Reset Uncommitted Changes",
                "Unstage all staged changes? The files keep their changes (git reset --mixed HEAD).",
                "Unstage",
            ),
            Self::Clean => (
                "Clean Untracked Files",
                "Delete all untracked files and directories? This can't be undone (git clean -d --force). Ignored files are kept.",
                "Delete",
            ),
            Self::Stash { .. }
            | Self::StashStaged { .. }
            | Self::OpenSourceControl
            | Self::Stage { .. }
            | Self::Unstage { .. }
            | Self::Commit { .. }
            | Self::UndoLastCommit
            | Self::Discard { .. } => return None,
        };
        Some(Confirmation::new(title, message.to_owned(), label))
    }

    #[must_use]
    pub fn inputs(&self) -> Vec<InputSpec> {
        match self {
            Self::Stash {
                message,
                include_untracked,
            } => vec![
                InputSpec::text(
                    InputId::Message,
                    "Message (optional)",
                    InputKind::Text,
                    message,
                ),
                InputSpec::toggle(
                    InputId::IncludeUntracked,
                    "Include untracked files",
                    *include_untracked,
                ),
            ],
            Self::StashStaged { message } => vec![InputSpec::text(
                InputId::Message,
                "Message (optional)",
                InputKind::Text,
                message,
            )],
            Self::Reset { mode } => vec![InputSpec::text(
                InputId::Mode,
                "Mode",
                InputKind::Choice(RESET_CHOICES),
                mode.name(),
            )],
            Self::Clean
            | Self::OpenSourceControl
            | Self::Stage { .. }
            | Self::Unstage { .. }
            | Self::Commit { .. }
            | Self::UndoLastCommit
            | Self::Discard { .. }
            | Self::DiscardAll => Vec::new(),
        }
    }

    pub fn set_input(&mut self, id: InputId, value: InputValue) -> Result<(), InputError> {
        match (self, id) {
            (Self::Stash { message, .. } | Self::StashStaged { message }, InputId::Message) => {
                *message = input::text(id, value)?;
            }
            (
                Self::Stash {
                    include_untracked, ..
                },
                InputId::IncludeUntracked,
            ) => *include_untracked = input::toggle(id, &value)?,
            (Self::Reset { mode }, InputId::Mode) => {
                let name = input::text(id, value)?;
                *mode = match name.as_str() {
                    "mixed" => ResetMode::Mixed,
                    "hard" => ResetMode::Hard,
                    _ => return Err(InputError::NotAChoice(name)),
                };
            }
            _ => return Err(InputError::NotAnInput(id)),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

/// `prefix` followed by `paths` as literal pathspecs (no globs, no magic).
fn with_paths(prefix: &[&str], paths: &[String]) -> Vec<String> {
    let mut args = strings(prefix);
    args.extend(paths.iter().map(|p| format!(":(literal){p}")));
    args
}
