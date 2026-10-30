//! The conflict banner: a merge, rebase, cherry-pick or revert git stopped
//! in the middle of, its Continue and Abort buttons, and the actions that
//! must wait until it is finished (AGENTS.md safety rules).

use ggt_actions::{Action, Operation, OperationAction};
use ggt_git::in_progress;
use ggt_ui::{Banner, BannerButton};

use super::{AppState, Command};

impl AppState {
    /// Reads the result of the in-progress check. Checking which marker
    /// exists looks at the disk: git only prints where the markers would be.
    pub(super) fn on_in_progress(&mut self, output: &str) {
        let repo = self.repo.clone();
        self.operation = in_progress::parse(output, |path| repo.join(path).exists());
    }

    /// The banner to show, while an operation is in progress.
    pub fn banner(&self) -> Option<Banner> {
        let operation = self.operation?;
        let conflicts = match self.status.conflicted {
            0 => "no conflicts left".to_owned(),
            1 => "1 conflicted file".to_owned(),
            n => format!("{n} conflicted files"),
        };
        Some(Banner::new(format!(
            "{} in progress · {conflicts}",
            operation.label()
        )))
    }

    /// Why `action` can't run now, if it can't: Continue waits for every
    /// conflict to be resolved, and actions that would start another
    /// operation (or move HEAD) wait for this one to finish.
    pub(super) fn refused_during_operation(&self, action: &Action) -> Option<String> {
        let operation = self.operation?;
        let conflicted = self.status.conflicted;
        match action {
            Action::Operation(OperationAction::Continue(_)) if conflicted > 0 => {
                Some(if conflicted == 1 {
                    "Resolve the conflicted file and stage it first.".to_owned()
                } else {
                    format!("Resolve the {conflicted} conflicted files and stage them first.")
                })
            }
            _ if action.conflicts_with_operation() => Some(format!(
                "A {} is in progress: continue or abort it first.",
                operation.label().to_lowercase()
            )),
            _ => None,
        }
    }

    /// `C` or the banner's `[Continue]`.
    pub(super) fn continue_operation(&mut self) -> Vec<Command> {
        self.operation_action(OperationAction::Continue, "Continue")
    }

    /// `A` or the banner's `[Abort]`: asks first.
    pub(super) fn abort_operation(&mut self) -> Vec<Command> {
        self.operation_action(OperationAction::Abort, "Abort")
    }

    fn operation_action(
        &mut self,
        action: fn(Operation) -> OperationAction,
        verb: &str,
    ) -> Vec<Command> {
        let Some(operation) = self.operation else {
            return Vec::new();
        };
        let label = format!("{verb} {}", operation.label());
        self.start_action(Action::Operation(action(operation)), label)
    }

    /// A left click on the banner; None when it missed the buttons.
    pub(super) fn on_banner_click(&mut self, x: u16, y: u16) -> Option<Vec<Command>> {
        Some(match self.banner_hits.hit(x, y)? {
            BannerButton::Continue => self.continue_operation(),
            BannerButton::Abort => self.abort_operation(),
        })
    }
}
