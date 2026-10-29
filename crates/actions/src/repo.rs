use std::path::Path;

use tgg_git::GitCommand;

use crate::action::{Action, Confirmation, Plan, strings};
use crate::input::{self, InputError, InputId, InputKind, InputSpec, InputValue};

/// What a ref picker opened from the repository menu is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickPurpose {
    /// "Checkout to...": any branch, remote branch or tag.
    Checkout,
    /// Branch ▸ "Delete Branch...": a local branch.
    DeleteBranch,
    /// Branch ▸ "Merge...": a branch to merge into the current one.
    Merge,
    /// Branch ▸ "Rebase Branch...": a branch to rebase the current one on.
    Rebase,
    /// Branch ▸ "Create Branch From...": the branch or tag to start from.
    CreateBranchFrom,
    /// Branch ▸ "Delete Remote Branch...": a remote branch.
    DeleteRemoteBranch,
    /// Stash ▸ "Apply Stash...", "Pop Stash...", "Drop Stash..." and
    /// "View Stash...": a stash.
    ApplyStash,
    PopStash,
    DropStash,
    ViewStash,
    /// Tags ▸ "Delete Tag..." and "Delete Remote Tag...": a tag.
    DeleteTag,
    DeleteRemoteTag,
    /// Remote ▸ "Remove Remote...": a remote.
    RemoveRemote,
}

impl PickPurpose {
    /// The action for the picked name, when the name alone decides it.
    /// Purposes that depend on the picked ref's kind or commit, or on the
    /// picked stash or the default remote, return `None`: the app builds
    /// those from what was picked.
    #[must_use]
    pub fn action_for(self, name: &str) -> Option<Action> {
        match self {
            Self::Checkout
            | Self::Merge
            | Self::Rebase
            | Self::CreateBranchFrom
            | Self::DeleteRemoteBranch
            | Self::ApplyStash
            | Self::PopStash
            | Self::DropStash
            | Self::ViewStash
            | Self::DeleteTag
            | Self::DeleteRemoteTag => None,
            Self::DeleteBranch => Some(Action::DeleteBranch {
                name: name.to_owned(),
                force: false,
            }),
            Self::RemoveRemote => Some(Action::Repo(RepoAction::RemoveRemote {
                name: name.to_owned(),
            })),
        }
    }
}

/// Repository-wide actions from the "..." menu (S11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoAction {
    /// Fetches every remote.
    Fetch {
        prune: bool,
    },
    /// VS Code's Sync: `git pull`, then `git push`, as configured.
    Sync,
    /// `git pull` from the upstream as git is configured (`pull.rebase`,
    /// `pull.ff`), or with `--rebase`; no dialog, as in VS Code.
    PullUpstream {
        rebase: bool,
    },
    /// Pulls `branch` from `remote` into the current branch.
    PullFrom {
        remote: String,
        branch: String,
    },
    /// `git push` to the upstream, as configured.
    Push,
    /// Deletes `branch` on `remote` (`git push --delete`).
    DeleteRemoteBranch {
        remote: String,
        branch: String,
    },
    /// `git fetch` from the default remote, optionally pruning.
    FetchDefault {
        prune: bool,
    },
    /// Deletes remote-tracking branches whose remote branch is gone.
    PruneRemotes {
        remotes: Vec<String>,
    },
    AddRemote {
        name: String,
        url: String,
    },
    RemoveRemote {
        name: String,
    },
    PushTags {
        remote: String,
    },
    ApplyLatestStash,
    PopLatestStash,
    /// Deletes every stash (`git stash clear`).
    DropAllStashes,
}

impl RepoAction {
    #[must_use]
    pub fn plan(&self, repo: &Path) -> Plan {
        let network = |args: Vec<String>| GitCommand::network(repo, args);
        let write = |args: Vec<String>| GitCommand::write(repo, args);
        let commands = match self {
            Self::Fetch { prune } => {
                let mut args = strings(&["fetch", "--all"]);
                if *prune {
                    args.push("--prune".into());
                }
                vec![network(args)]
            }
            Self::Sync => vec![network(strings(&["pull"])), network(strings(&["push"]))],
            Self::PullUpstream { rebase } => {
                let mut args = strings(&["pull"]);
                if *rebase {
                    args.push("--rebase".into());
                }
                vec![network(args)]
            }
            Self::PullFrom { remote, branch } => {
                vec![network(strings(&["pull", "--", remote, branch]))]
            }
            Self::Push => vec![network(strings(&["push"]))],
            Self::DeleteRemoteBranch { remote, branch } => {
                let refspec = format!("refs/heads/{branch}");
                vec![network(strings(&[
                    "push", "--delete", "--", remote, &refspec,
                ]))]
            }
            Self::FetchDefault { prune } => {
                let mut args = strings(&["fetch"]);
                if *prune {
                    args.push("--prune".into());
                }
                vec![network(args)]
            }
            Self::PruneRemotes { remotes } => remotes
                .iter()
                .map(|r| network(strings(&["remote", "prune", "--", r])))
                .collect(),
            Self::AddRemote { name, url } => {
                vec![write(strings(&["remote", "add", "--", name, url]))]
            }
            Self::RemoveRemote { name } => vec![write(strings(&["remote", "remove", "--", name]))],
            Self::PushTags { remote } => vec![network(strings(&["push", "--tags", "--", remote]))],
            Self::ApplyLatestStash => vec![write(strings(&["stash", "apply"]))],
            Self::PopLatestStash => vec![write(strings(&["stash", "pop"]))],
            Self::DropAllStashes => vec![write(strings(&["stash", "clear"]))],
        };
        Plan::Git(commands)
    }

    #[must_use]
    pub fn is_destructive(&self) -> bool {
        matches!(
            self,
            Self::RemoveRemote { .. } | Self::DeleteRemoteBranch { .. } | Self::DropAllStashes
        )
    }

    #[must_use]
    pub fn confirmation(&self) -> Option<Confirmation> {
        if let Self::DropAllStashes = self {
            return Some(Confirmation::new(
                "Drop All Stashes",
                "Drop EVERY stash? Their changes are deleted (git stash clear).".to_owned(),
                "Drop All",
            ));
        }
        if let Self::DeleteRemoteBranch { remote, branch } = self {
            return Some(Confirmation::new(
                "Delete Remote Branch",
                format!(
                    "Delete the branch '{branch}' on '{remote}'? Everyone who fetches from '{remote}' loses it (git push --delete {remote} refs/heads/{branch})."
                ),
                "Delete",
            ));
        }
        let Self::RemoveRemote { name } = self else {
            return None;
        };
        Some(Confirmation::new(
            "Remove Remote",
            format!(
                "Remove the remote '{name}'? Its remote-tracking branches ({name}/*) are deleted here; the remote repository is not touched."
            ),
            "Remove",
        ))
    }

    #[must_use]
    pub fn inputs(&self) -> Vec<InputSpec> {
        match self {
            Self::PullFrom { remote, branch } => vec![
                InputSpec::text(InputId::Remote, "Remote", InputKind::Remote, remote),
                InputSpec::text(InputId::Name, "Branch", InputKind::RefName, branch),
            ],
            Self::AddRemote { name, url } => vec![
                InputSpec::text(InputId::Name, "Remote name", InputKind::Remote, name),
                InputSpec::text(InputId::Url, "URL", InputKind::Path, url),
            ],
            _ => Vec::new(),
        }
    }

    pub fn set_input(&mut self, id: InputId, value: InputValue) -> Result<(), InputError> {
        match (self, id) {
            (Self::PullFrom { remote, .. }, InputId::Remote) => {
                *remote = input::ref_name(id, value)?;
            }
            (Self::PullFrom { branch, .. }, InputId::Name) => *branch = input::ref_name(id, value)?,
            (Self::AddRemote { name, .. }, InputId::Name) => *name = input::ref_name(id, value)?,
            (Self::AddRemote { url, .. }, InputId::Url) => *url = input::required(id, value)?,
            _ => return Err(InputError::NotAnInput(id)),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
