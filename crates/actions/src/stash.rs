use std::path::Path;

use tgg_git::GitCommand;
use tgg_model::Stash;

use crate::action::{Confirmation, Plan, strings};
use crate::input::{self, InputError, InputId, InputKind, InputSpec, InputValue};

/// Actions on a stash entry.
///
/// Stashes are addressed by selector (`stash@{n}`), which `drop` and `pop`
/// require. It is taken when the menu opens; the app refreshes after every
/// action, so the index is current.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StashAction {
    /// `reinstate_index` also restores what was staged (`--index`).
    Apply {
        stash: Stash,
        reinstate_index: bool,
    },
    /// Apply, then drop the stash if that succeeded.
    Pop {
        stash: Stash,
        reinstate_index: bool,
    },
    Drop {
        stash: Stash,
    },
    /// `git stash branch`: a new branch at the stash's base with its changes.
    CreateBranch {
        stash: Stash,
        name: String,
    },
    CopyName {
        stash: Stash,
    },
}

impl StashAction {
    #[must_use]
    pub fn plan(&self, repo: &Path) -> Plan {
        let args = match self {
            Self::Apply {
                stash,
                reinstate_index,
            } => restore("apply", stash, *reinstate_index),
            Self::Pop {
                stash,
                reinstate_index,
            } => restore("pop", stash, *reinstate_index),
            Self::Drop { stash } => strings(&["stash", "drop", "--", &stash.selector()]),
            Self::CreateBranch { stash, name } => {
                strings(&["stash", "branch", "--", name, &stash.selector()])
            }
            Self::CopyName { stash } => return Plan::Clipboard(stash.selector()),
        };
        Plan::Git(vec![GitCommand::write(repo, args)])
    }

    #[must_use]
    pub fn is_destructive(&self) -> bool {
        matches!(self, Self::Drop { .. })
    }

    #[must_use]
    pub fn confirmation(&self) -> Option<Confirmation> {
        let Self::Drop { stash } = self else {
            return None;
        };
        Some(Confirmation::new(
            "Drop Stash",
            format!(
                "Drop {} \"{}\"? Its changes are deleted.",
                stash.selector(),
                stash.subject
            ),
            "Drop",
        ))
    }

    #[must_use]
    pub fn inputs(&self) -> Vec<InputSpec> {
        match self {
            Self::Apply {
                reinstate_index, ..
            }
            | Self::Pop {
                reinstate_index, ..
            } => vec![InputSpec::toggle(
                InputId::ReinstateIndex,
                "Reinstate the staged changes (--index)",
                *reinstate_index,
            )],
            Self::CreateBranch { name, .. } => vec![InputSpec::text(
                InputId::Name,
                "Branch name",
                InputKind::RefName,
                name,
            )],
            Self::Drop { .. } | Self::CopyName { .. } => Vec::new(),
        }
    }

    pub fn set_input(&mut self, id: InputId, value: InputValue) -> Result<(), InputError> {
        match (self, id) {
            (
                Self::Apply {
                    reinstate_index, ..
                }
                | Self::Pop {
                    reinstate_index, ..
                },
                InputId::ReinstateIndex,
            ) => *reinstate_index = input::toggle(id, &value)?,
            (Self::CreateBranch { name, .. }, InputId::Name) => *name = input::ref_name(id, value)?,
            _ => return Err(InputError::NotAnInput(id)),
        }
        Ok(())
    }
}

fn restore(verb: &str, stash: &Stash, reinstate_index: bool) -> Vec<String> {
    let mut args = strings(&["stash", verb]);
    if reinstate_index {
        args.push("--index".into());
    }
    args.extend(strings(&["--", &stash.selector()]));
    args
}

#[cfg(test)]
mod tests;
