//! Running actions: input dialog → confirmation → git steps → refresh.

use std::collections::VecDeque;

use crossterm::event::{KeyCode, KeyEvent};
use tgg_actions::{Action, InputId, InputValue, Plan, UiEffect};
use tgg_git::{ExecError, Failure, GitCommand, Output};
use tgg_ui::{
    ConfirmDialog, DialogHitMap, DialogOutcome, InputDialog, InputHitMap, InputOutcome, Key,
};

use super::{AppState, Command};

/// A modal dialog on top of everything else.
#[derive(Debug)]
pub enum Dialog {
    /// Asks for the action's inputs (names, options).
    Input {
        view: InputDialog,
        action: Action,
        label: String,
    },
    /// Asks to confirm the action (spec S13).
    Confirm {
        view: ConfirmDialog,
        action: Action,
        label: String,
    },
}

impl Dialog {
    fn into_parts(self) -> (Action, String) {
        match self {
            Self::Input { action, label, .. } | Self::Confirm { action, label, .. } => {
                (action, label)
            }
        }
    }

    fn label(&self) -> String {
        match self {
            Self::Input { label, .. } | Self::Confirm { label, .. } => label.clone(),
        }
    }
}

/// The open dialog's hit-map from the last frame.
#[derive(Debug, Clone)]
pub enum DialogHits {
    Input(InputHitMap),
    Confirm(DialogHitMap),
}

/// An action whose git steps are running, one at a time.
#[derive(Debug)]
pub(super) struct Running {
    label: String,
    action: Action,
    remaining: VecDeque<GitCommand>,
}

/// What a dialog answered.
enum Step {
    Stay,
    Cancel,
    /// Inputs are complete; confirm (if needed) and run.
    Submitted(Action),
    Confirmed,
}

impl AppState {
    /// Starts an action chosen from a menu: asks for inputs, then confirms,
    /// then runs it. Only one action runs at a time.
    pub fn start_action(&mut self, action: Action, label: String) -> Vec<Command> {
        if self.running.is_some() {
            self.message = Some("Wait for the current action to finish.".into());
            return Vec::new();
        }
        if let Some(view) = InputDialog::for_action(&label, &action) {
            let view = view.with_remotes(&self.menu_context().remotes);
            self.open_dialog(Dialog::Input {
                view,
                action,
                label,
            });
            return Vec::new();
        }
        self.confirm_or_run(action, label)
    }

    fn confirm_or_run(&mut self, action: Action, label: String) -> Vec<Command> {
        if let Some(view) = ConfirmDialog::for_action(&action) {
            self.open_dialog(Dialog::Confirm {
                view,
                action,
                label,
            });
            return Vec::new();
        }
        self.run_action(action, label)
    }

    fn open_dialog(&mut self, dialog: Dialog) {
        self.dialog = Some(dialog);
        self.dialog_hits = None;
    }

    fn run_action(&mut self, action: Action, label: String) -> Vec<Command> {
        match action.plan(&self.repo) {
            Plan::Git(commands) => {
                let mut remaining: VecDeque<GitCommand> = commands.into();
                let Some(first) = remaining.pop_front() else {
                    return Vec::new();
                };
                self.message = Some(format!("{label}…"));
                self.running = Some(Running {
                    label,
                    action,
                    remaining,
                });
                vec![Command::Run(first)]
            }
            Plan::Clipboard(text) => {
                self.message = Some(format!("Copied to the clipboard: {text}"));
                vec![Command::Clipboard(text)]
            }
            Plan::Ui(UiEffect::ToggleBranchFilter { branch }) => {
                self.message = Some(format!(
                    "Filtering by {branch} comes with the branch filter (toolbar)."
                ));
                Vec::new()
            }
            Plan::Ui(UiEffect::ShowTag { name }) => self.show_tag(name),
            Plan::Ui(UiEffect::OpenSourceControl) => {
                self.message =
                    Some("Use `git status` or your editor's source control view.".to_owned());
                Vec::new()
            }
            Plan::Ui(UiEffect::PickRef { .. }) => {
                self.message = Some("Picking a ref comes with the picker (toolbar).".to_owned());
                Vec::new()
            }
            Plan::Ui(UiEffect::ShowGitOutput) => self.show_git_output(),
        }
    }

    /// The result of one git step of the running action.
    pub(super) fn on_action_result(
        &mut self,
        command: &GitCommand,
        result: Result<Output, ExecError>,
    ) -> Vec<Command> {
        self.record_output(command, &result);
        let Some(mut running) = self.running.take() else {
            return Vec::new();
        };
        match result {
            Ok(_) => {
                if let Some(next) = running.remaining.pop_front() {
                    self.running = Some(running);
                    return vec![Command::Run(next)];
                }
                self.message = Some(format!("✓ {}", running.label));
                self.refresh()
            }
            Err(err) => {
                // `branch -d` refuses unmerged branches: offer a force delete.
                if let (
                    ExecError::Failed {
                        failure: Failure::NotFullyMerged,
                        ..
                    },
                    Action::DeleteBranch { force: false, .. },
                ) = (&err, &running.action)
                {
                    let mut forced = running.action.clone();
                    if forced
                        .set_input(InputId::Force, InputValue::Bool(true))
                        .is_ok()
                    {
                        self.message = Some(err.user_message());
                        return self.confirm_or_run(forced, "Force Delete Branch".into());
                    }
                }
                self.message = Some(format!("{} failed: {}", running.label, err.user_message()));
                // Earlier steps may have changed the repository.
                self.refresh()
            }
        }
    }

    pub(super) fn on_dialog_key(&mut self, key: KeyEvent) -> Vec<Command> {
        let Some(key) = widget_key(key) else {
            return Vec::new();
        };
        let Some(mut dialog) = self.dialog.take() else {
            return Vec::new();
        };
        let step = match &mut dialog {
            Dialog::Input { view, .. } => input_step(view.handle_key(key)),
            Dialog::Confirm { view, .. } => confirm_step(view.handle_key(key)),
        };
        self.apply_step(dialog, step)
    }

    pub(super) fn on_dialog_click(&mut self, x: u16, y: u16) -> Vec<Command> {
        let Some(mut dialog) = self.dialog.take() else {
            return Vec::new();
        };
        let step = match (&mut dialog, self.dialog_hits.as_ref()) {
            (Dialog::Input { view, .. }, Some(DialogHits::Input(hits))) => {
                input_step(view.handle_click(hits, x, y))
            }
            (Dialog::Confirm { view, .. }, Some(DialogHits::Confirm(hits))) => {
                confirm_step(view.handle_click(hits, x, y))
            }
            _ => Step::Stay,
        };
        self.apply_step(dialog, step)
    }

    fn apply_step(&mut self, dialog: Dialog, step: Step) -> Vec<Command> {
        match step {
            Step::Stay => {
                self.dialog = Some(dialog);
                Vec::new()
            }
            Step::Cancel => {
                self.dialog_hits = None;
                self.message = Some(format!("{} cancelled.", dialog.label()));
                Vec::new()
            }
            Step::Submitted(action) => {
                self.dialog_hits = None;
                self.confirm_or_run(action, dialog.label())
            }
            Step::Confirmed => {
                self.dialog_hits = None;
                let (action, label) = dialog.into_parts();
                self.run_action(action, label)
            }
        }
    }
}

fn input_step(outcome: InputOutcome) -> Step {
    match outcome {
        InputOutcome::Pending => Step::Stay,
        InputOutcome::Cancelled => Step::Cancel,
        InputOutcome::Submitted(action) => Step::Submitted(action),
    }
}

fn confirm_step(outcome: DialogOutcome) -> Step {
    match outcome {
        DialogOutcome::Pending => Step::Stay,
        DialogOutcome::Cancelled => Step::Cancel,
        DialogOutcome::Confirmed => Step::Confirmed,
    }
}

/// Maps a terminal key to the widget key set, unchanged (dialogs need every letter).
pub(super) fn widget_key(key: KeyEvent) -> Option<Key> {
    Some(match key.code {
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Esc,
        KeyCode::Tab => Key::Tab,
        KeyCode::BackTab => Key::BackTab,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Delete => Key::Delete,
        KeyCode::Char(c) => Key::Char(c),
        _ => return None,
    })
}
