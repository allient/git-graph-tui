use std::path::Path;

use tgg_git::GitCommand;

use crate::action::{Confirmation, Plan, ResetMode, UiEffect, strings};
use crate::input::{self, InputError, InputId, InputKind, InputSpec, InputValue};

/// Reset modes that make sense for uncommitted changes (soft would do nothing).
const RESET_CHOICES: &[&str] = &["mixed", "hard"];

/// Actions on the "Uncommitted Changes" row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkingTreeAction {
    /// An empty `message` lets git write `WIP on <branch>: ...`.
    Stash {
        message: String,
        include_untracked: bool,
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
    /// Commits what is staged.
    Commit {
        message: String,
    },
}

impl WorkingTreeAction {
    #[must_use]
    pub fn plan(&self, repo: &Path) -> Plan {
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
            // `--` after HEAD: what follows a revision for reset are paths.
            Self::Reset { mode } => {
                strings(&["reset", &format!("--{}", mode.name()), "HEAD", "--"])
            }
            Self::Clean => strings(&["clean", "-d", "--force", "--"]),
            Self::OpenSourceControl => return Plan::Ui(UiEffect::OpenSourceControl),
            Self::Stage { paths } => with_paths(&["add", "--all", "--"], paths),
            Self::Unstage { paths } => with_paths(&["reset", "--quiet", "--"], paths),
            // One `--message=` argument, so a message starting with `-` stays a message.
            Self::Commit { message } => vec!["commit".into(), format!("--message={message}")],
        };
        Plan::Git(vec![GitCommand::write(repo, args)])
    }

    #[must_use]
    pub fn is_destructive(&self) -> bool {
        matches!(
            self,
            Self::Reset {
                mode: ResetMode::Hard
            } | Self::Clean
        )
    }

    #[must_use]
    pub fn confirmation(&self) -> Option<Confirmation> {
        let (title, message, label) = match self {
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
            | Self::OpenSourceControl
            | Self::Stage { .. }
            | Self::Unstage { .. }
            | Self::Commit { .. } => return None,
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
            | Self::Commit { .. } => Vec::new(),
        }
    }

    pub fn set_input(&mut self, id: InputId, value: InputValue) -> Result<(), InputError> {
        match (self, id) {
            (Self::Stash { message, .. }, InputId::Message) => *message = input::text(id, value)?,
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
