use std::path::Path;

use ggt_git::GitCommand;

use crate::action::{Confirmation, Plan, strings};
use crate::input::{self, InputError, InputId, InputKind, InputSpec, InputValue};

/// Actions of the worktrees view (S20 4). Worktrees are addressed by the
/// path `git worktree list` printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorktreeAction {
    /// `git worktree add`: checks out `branch` in a new folder. An empty
    /// `path` means `../<repository folder>-<branch>`.
    Add {
        path: String,
        branch: String,
        /// Create `branch` at HEAD (`-b`) rather than check out an existing one.
        new_branch: bool,
    },
    /// `git worktree remove`: deletes the folder and git's record of it.
    /// `force` removes it even with uncommitted changes, or when `locked`.
    Remove {
        path: String,
        force: bool,
        locked: bool,
    },
    /// Forgets worktrees whose folder is gone.
    Prune,
    Lock {
        path: String,
    },
    Unlock {
        path: String,
    },
    CopyPath {
        path: String,
    },
}

impl WorktreeAction {
    /// "Add Worktree...", with every field to fill in.
    #[must_use]
    pub fn add() -> Self {
        Self::Add {
            path: String::new(),
            branch: String::new(),
            new_branch: true,
        }
    }

    #[must_use]
    pub fn plan(&self, repo: &Path) -> Plan {
        let args = match self {
            Self::Add {
                path,
                branch,
                new_branch,
            } => {
                let path = if path.trim().is_empty() {
                    default_path(repo, branch)
                } else {
                    path.clone()
                };
                let mut args = strings(&["worktree", "add"]);
                if *new_branch {
                    args.extend(strings(&["-b", branch, "--", &path]));
                } else {
                    args.extend(strings(&["--", &path, branch]));
                }
                args
            }
            Self::Remove {
                path,
                force,
                locked,
            } => {
                let mut args = strings(&["worktree", "remove"]);
                if *force {
                    args.push("--force".into());
                    // git asks for it twice to remove a locked worktree.
                    if *locked {
                        args.push("--force".into());
                    }
                }
                args.extend(strings(&["--", path]));
                args
            }
            Self::Prune => strings(&["worktree", "prune"]),
            Self::Lock { path } => strings(&["worktree", "lock", "--", path]),
            Self::Unlock { path } => strings(&["worktree", "unlock", "--", path]),
            Self::CopyPath { path } => return Plan::Clipboard(path.clone()),
        };
        Plan::Git(vec![GitCommand::write(repo, args)])
    }

    #[must_use]
    pub fn is_destructive(&self) -> bool {
        matches!(self, Self::Remove { .. })
    }

    #[must_use]
    pub fn confirmation(&self) -> Option<Confirmation> {
        let Self::Remove {
            path,
            force,
            locked,
        } = self
        else {
            return None;
        };
        let message = if *force {
            let locked = if *locked {
                ", even though it is locked"
            } else {
                ""
            };
            format!(
                "Force remove the worktree at '{path}'{locked}? Its folder is deleted, uncommitted changes and untracked files included."
            )
        } else {
            format!(
                "Remove the worktree at '{path}'? Its folder is deleted; git refuses if it has uncommitted changes or is locked."
            )
        };
        let label = if *force { "Force Remove" } else { "Remove" };
        Some(Confirmation::new("Remove Worktree", message, label))
    }

    #[must_use]
    pub fn inputs(&self) -> Vec<InputSpec> {
        match self {
            Self::Add {
                path,
                branch,
                new_branch,
            } => vec![
                InputSpec::text(InputId::Name, "Branch", InputKind::RefName, branch),
                InputSpec::toggle(InputId::NewBranch, "Create a new branch", *new_branch),
                InputSpec::text(
                    InputId::Path,
                    "Folder (empty for ../<repository>-<branch>)",
                    InputKind::Text,
                    path,
                ),
            ],
            Self::Remove { force, .. } => vec![InputSpec::toggle(
                InputId::Force,
                "Force, even with uncommitted changes or a lock",
                *force,
            )],
            Self::Prune | Self::Lock { .. } | Self::Unlock { .. } | Self::CopyPath { .. } => {
                Vec::new()
            }
        }
    }

    pub fn set_input(&mut self, id: InputId, value: InputValue) -> Result<(), InputError> {
        match (self, id) {
            (Self::Add { branch, .. }, InputId::Name) => *branch = input::ref_name(id, value)?,
            (Self::Add { new_branch, .. }, InputId::NewBranch) => {
                *new_branch = input::toggle(id, &value)?;
            }
            (Self::Add { path, .. }, InputId::Path) => *path = input::text(id, value)?,
            (Self::Remove { force, .. }, InputId::Force) => *force = input::toggle(id, &value)?,
            _ => return Err(InputError::NotAnInput(id)),
        }
        Ok(())
    }
}

/// `../<repository folder>-<branch>`, with the branch's slashes as dashes
/// so `feature/x` stays one folder.
fn default_path(repo: &Path, branch: &str) -> String {
    let name = repo
        .file_name()
        .map_or_else(|| "worktree".into(), |n| n.to_string_lossy());
    format!("../{name}-{}", branch.replace('/', "-"))
}

#[cfg(test)]
mod tests;
