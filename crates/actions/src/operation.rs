use std::path::Path;

use tgg_git::GitCommand;
pub use tgg_git::in_progress::Operation;

use crate::action::{Confirmation, Plan};

/// The conflict banner's buttons: finish or undo the merge, rebase,
/// cherry-pick or revert git stopped in the middle of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationAction {
    /// `git <operation> --continue`, keeping git's prepared message. The app
    /// only offers it once no file is left conflicted.
    Continue(Operation),
    /// `git <operation> --abort`: back to before the operation started.
    Abort(Operation),
}

impl OperationAction {
    #[must_use]
    pub fn plan(self, repo: &Path) -> Plan {
        let command = match self {
            Self::Continue(operation) => {
                GitCommand::write(repo, [operation.subcommand(), "--continue"]).without_editor()
            }
            Self::Abort(operation) => GitCommand::write(repo, [operation.subcommand(), "--abort"]),
        };
        Plan::Git(vec![command])
    }

    #[must_use]
    pub fn is_destructive(self) -> bool {
        matches!(self, Self::Abort(_))
    }

    #[must_use]
    pub fn confirmation(self) -> Option<Confirmation> {
        let Self::Abort(operation) = self else {
            return None;
        };
        let name = operation.subcommand();
        let what = match operation {
            Operation::Merge => {
                "The working tree and index go back to how they were before it started: your conflict resolutions are lost."
            }
            Operation::Rebase => {
                "The branch goes back to where it was before the rebase started: the commits already replayed and your conflict resolutions are lost."
            }
            Operation::CherryPick | Operation::Revert => {
                "HEAD, the working tree and index go back to how they were before it started: your conflict resolutions are lost."
            }
        };
        Some(Confirmation::new(
            &format!("Abort {}", operation.label()),
            format!("Abort the {name} (git {name} --abort)? {what}"),
            "Abort",
        ))
    }
}

#[cfg(test)]
mod tests;
