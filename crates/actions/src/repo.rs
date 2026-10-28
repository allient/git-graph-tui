use std::path::Path;

use tgg_git::GitCommand;

use crate::action::{Action, Confirmation, Plan, strings};
use crate::input::{self, InputError, InputId, InputKind, InputSpec, InputValue};

/// How `Pull` combines remote commits with local ones.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PullMode {
    /// Refuse when the branches diverged (`--ff-only`): never creates a
    /// merge or rewrites anything behind the user's back.
    #[default]
    FastForwardOnly,
    Rebase,
    /// Merge commit when needed (`--no-rebase`).
    Merge,
}

impl PullMode {
    pub const CHOICES: &'static [&'static str] = &["ff-only", "rebase", "merge"];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::FastForwardOnly => "ff-only",
            Self::Rebase => "rebase",
            Self::Merge => "merge",
        }
    }

    fn flag(self) -> &'static str {
        match self {
            Self::FastForwardOnly => "--ff-only",
            Self::Rebase => "--rebase",
            Self::Merge => "--no-rebase",
        }
    }
}

/// What a ref picker opened from the repository menu is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickPurpose {
    /// "Checkout to...": any branch, remote branch or tag.
    Checkout,
    /// Branch ▸ "Delete Branch...": a local branch.
    DeleteBranch,
    /// Remote ▸ "Remove Remote...": a remote.
    RemoveRemote,
}

impl PickPurpose {
    /// The action for the picked name, when the name alone decides it.
    /// `Checkout` returns `None`: local, remote and tag checkouts differ, so
    /// the app builds it from the picked ref (e.g. via `Target::from_ref`).
    #[must_use]
    pub fn action_for(self, name: &str) -> Option<Action> {
        match self {
            Self::Checkout => None,
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
    /// Pulls the current branch (`branch`, for messages) from its upstream.
    Pull {
        branch: String,
        mode: PullMode,
    },
    /// Fetches every remote.
    Fetch {
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
}

impl RepoAction {
    #[must_use]
    pub fn plan(&self, repo: &Path) -> Plan {
        let network = |args: Vec<String>| GitCommand::network(repo, args);
        let write = |args: Vec<String>| GitCommand::write(repo, args);
        let commands = match self {
            Self::Pull { mode, .. } => vec![network(strings(&["pull", mode.flag()]))],
            Self::Fetch { prune } => {
                let mut args = strings(&["fetch", "--all"]);
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
        };
        Plan::Git(commands)
    }

    #[must_use]
    pub fn is_destructive(&self) -> bool {
        matches!(self, Self::RemoveRemote { .. })
    }

    #[must_use]
    pub fn confirmation(&self) -> Option<Confirmation> {
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
            Self::Pull { mode, .. } => vec![InputSpec::text(
                InputId::Mode,
                "Mode",
                InputKind::Choice(PullMode::CHOICES),
                mode.name(),
            )],
            Self::AddRemote { name, url } => vec![
                InputSpec::text(InputId::Name, "Remote name", InputKind::Remote, name),
                InputSpec::text(InputId::Url, "URL", InputKind::Path, url),
            ],
            _ => Vec::new(),
        }
    }

    pub fn set_input(&mut self, id: InputId, value: InputValue) -> Result<(), InputError> {
        match (self, id) {
            (Self::Pull { mode, .. }, InputId::Mode) => {
                let name = input::text(id, value)?;
                *mode = match name.as_str() {
                    "ff-only" => PullMode::FastForwardOnly,
                    "rebase" => PullMode::Rebase,
                    "merge" => PullMode::Merge,
                    _ => return Err(InputError::NotAChoice(name)),
                };
            }
            (Self::AddRemote { name, .. }, InputId::Name) => *name = input::ref_name(id, value)?,
            (Self::AddRemote { url, .. }, InputId::Url) => *url = input::required(id, value)?,
            _ => return Err(InputError::NotAnInput(id)),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
