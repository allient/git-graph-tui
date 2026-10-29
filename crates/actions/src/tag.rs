use std::path::Path;

use tgg_git::GitCommand;

use crate::action::{Confirmation, Plan, UiEffect, strings};
use crate::input::{self, InputError, InputId, InputKind, InputSpec, InputValue};

/// Actions on a tag badge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagAction {
    /// Deletes the local tag and, when `on_remote`, the tag on `remote`
    /// (the default remote; `None` when the repository has none).
    Delete {
        name: String,
        remote: Option<String>,
        on_remote: bool,
    },
    Push {
        name: String,
        remote: String,
    },
    /// Deletes the tag on `remote` only (Tags ▸ "Delete Remote Tag...").
    DeleteRemote {
        name: String,
        remote: String,
    },
    /// Shows the tag (and an annotated tag's message) in the app.
    ShowDetails {
        name: String,
    },
    CopyName {
        name: String,
    },
}

impl TagAction {
    #[must_use]
    pub fn plan(&self, repo: &Path) -> Plan {
        match self {
            Self::Delete {
                name,
                remote,
                on_remote,
            } => {
                let mut commands = vec![GitCommand::write(
                    repo,
                    strings(&["tag", "--delete", "--", name]),
                )];
                if let (true, Some(remote)) = (*on_remote, remote) {
                    let refspec = format!("refs/tags/{name}");
                    commands.push(GitCommand::network(
                        repo,
                        strings(&["push", "--delete", "--", remote, &refspec]),
                    ));
                }
                Plan::Git(commands)
            }
            Self::Push { name, remote } => {
                let refspec = format!("refs/tags/{name}");
                Plan::Git(vec![GitCommand::network(
                    repo,
                    strings(&["push", "--", remote, &refspec]),
                )])
            }
            Self::DeleteRemote { name, remote } => {
                let refspec = format!("refs/tags/{name}");
                Plan::Git(vec![GitCommand::network(
                    repo,
                    strings(&["push", "--delete", "--", remote, &refspec]),
                )])
            }
            Self::ShowDetails { name } => Plan::Ui(UiEffect::ShowTag { name: name.clone() }),
            Self::CopyName { name } => Plan::Clipboard(name.clone()),
        }
    }

    #[must_use]
    pub fn is_destructive(&self) -> bool {
        matches!(self, Self::Delete { .. } | Self::DeleteRemote { .. })
    }

    #[must_use]
    pub fn confirmation(&self) -> Option<Confirmation> {
        if let Self::DeleteRemote { name, remote } = self {
            return Some(Confirmation::new(
                "Delete Remote Tag",
                format!(
                    "Delete the tag '{name}' on '{remote}'? The local tag is kept; everyone who fetches from '{remote}' loses it (git push --delete {remote} refs/tags/{name})."
                ),
                "Delete",
            ));
        }
        let Self::Delete {
            name,
            remote,
            on_remote,
        } = self
        else {
            return None;
        };
        let message = match (on_remote, remote) {
            (true, Some(remote)) => format!(
                "Delete the tag '{name}' here and on '{remote}'? Everyone who fetches from '{remote}' loses it (git push --delete {remote} refs/tags/{name})."
            ),
            _ => format!(
                "Delete the local tag '{name}'? Its commit is kept, and so is the tag on remotes."
            ),
        };
        Some(Confirmation::new("Delete Tag", message, "Delete"))
    }

    #[must_use]
    pub fn inputs(&self) -> Vec<InputSpec> {
        match self {
            Self::Delete {
                remote: Some(_),
                on_remote,
                ..
            } => vec![InputSpec::toggle(
                InputId::DeleteOnRemote,
                "Also delete it on the remote",
                *on_remote,
            )],
            Self::Push { remote, .. } => vec![InputSpec::text(
                InputId::Remote,
                "Remote",
                InputKind::Remote,
                remote,
            )],
            _ => Vec::new(),
        }
    }

    pub fn set_input(&mut self, id: InputId, value: InputValue) -> Result<(), InputError> {
        match (self, id) {
            (
                Self::Delete {
                    remote: Some(_),
                    on_remote,
                    ..
                },
                InputId::DeleteOnRemote,
            ) => *on_remote = input::toggle(id, &value)?,
            (Self::Push { remote, .. }, InputId::Remote) => *remote = input::ref_name(id, value)?,
            _ => return Err(InputError::NotAnInput(id)),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
