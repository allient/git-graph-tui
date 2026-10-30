//! The Source Control pane (S3): focus, keys, clicks and the git actions it runs.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ggt_actions::{Action, CommitScope, WorkingTreeAction};
use ggt_model::HeadState;
use ggt_ui::{Key, SourceControl, SourceControlHitMap, SourceControlOutcome};

use super::{AppState, Command, actions::widget_key};

/// Narrowest terminal that shows the pane beside the graph.
pub const MIN_WIDTH_FOR_PANE: u16 = 90;

/// The pane's widget, keyboard focus and last frame.
#[derive(Debug, Default)]
pub struct SourceControlState {
    pub view: SourceControl,
    /// The pane has the keyboard.
    pub focus: bool,
    pub hits: SourceControlHitMap,
    /// The last frame was wide enough to show the pane.
    pub visible: bool,
    /// Width set by dragging the divider; None uses the default.
    pub width: Option<u16>,
    /// Column of the divider as last drawn.
    pub divider: Option<u16>,
}

impl AppState {
    /// Gives the keyboard to the pane's message box (`c`, or "Open Source
    /// Control" in the uncommitted changes menu).
    pub(super) fn focus_source_control(&mut self) {
        if !self.source_control.visible {
            self.message = Some(format!(
                "Widen the terminal to at least {MIN_WIDTH_FOR_PANE} columns to show Source Control."
            ));
            return;
        }
        self.toolbar.focus = None;
        self.source_control.focus = true;
        self.source_control.view.focus_message();
    }

    /// Lists the files of a new `git status` in the pane.
    pub(super) fn update_source_control(&mut self) {
        let branch = match &self.status.head {
            HeadState::Branch(name) => Some(name.clone()),
            _ => None,
        };
        self.source_control
            .view
            .set_changes(self.status.changes.clone(), branch);
    }

    pub(super) fn on_source_control_key(&mut self, key: KeyEvent) -> Vec<Command> {
        // Alt+Enter, Shift+Enter (where the terminal reports it) and Ctrl+J
        // start a new line in the message; a plain Enter commits.
        let newline = match key.code {
            KeyCode::Enter => key
                .modifiers
                .intersects(KeyModifiers::ALT | KeyModifiers::SHIFT),
            KeyCode::Char('j') => key.modifiers.contains(KeyModifiers::CONTROL),
            _ => false,
        };
        let key = if newline {
            Key::Char('\n')
        } else {
            let Some(key) = widget_key(key) else {
                return Vec::new();
            };
            key
        };
        let outcome = self.source_control.view.handle_key(key);
        self.on_source_control_outcome(outcome)
    }

    /// A left click in the pane, which takes the keyboard; None when the click
    /// missed it (and the keyboard goes back to the graph).
    pub(super) fn on_source_control_click(&mut self, x: u16, y: u16) -> Option<Vec<Command>> {
        if !self.source_control.hits.contains(x, y) {
            self.source_control.focus = false;
            return None;
        }
        self.toolbar.focus = None;
        self.source_control.focus = true;
        let hits = self.source_control.hits.clone();
        let outcome = self.source_control.view.handle_click(&hits, x, y);
        Some(self.on_source_control_outcome(outcome))
    }

    fn on_source_control_outcome(&mut self, outcome: SourceControlOutcome) -> Vec<Command> {
        let action = match outcome {
            SourceControlOutcome::Stay => return Vec::new(),
            SourceControlOutcome::Leave => {
                self.source_control.focus = false;
                return Vec::new();
            }
            SourceControlOutcome::Refresh => {
                self.message = None;
                return self.refresh();
            }
            SourceControlOutcome::More(anchor) => {
                // What the menu does happens in the graph: keys go back there
                // instead of into the commit message.
                self.source_control.focus = false;
                self.open_repo_menu(anchor);
                return Vec::new();
            }
            SourceControlOutcome::OpenDiff(file) => return self.open_working_diff(&file),
            SourceControlOutcome::OpenChanges { staged, files } => {
                return self.open_section_diff(staged, files);
            }
            SourceControlOutcome::OpenFile(file) => return self.open_in_editor(&file),
            SourceControlOutcome::Discard { paths, untracked } => {
                ("Discard", WorkingTreeAction::Discard { paths, untracked })
            }
            SourceControlOutcome::Stage(paths) => ("Stage", WorkingTreeAction::Stage { paths }),
            SourceControlOutcome::Unstage(paths) => {
                ("Unstage", WorkingTreeAction::Unstage { paths })
            }
            SourceControlOutcome::Commit(_) => {
                return self.commit(CommitScope::Staged, false, false);
            }
        };
        let (label, action) = action;
        self.start_action(Action::WorkingTree(action), label.into())
    }

    /// Commits with the pane's message: the pane's `✓` (what is staged) and
    /// the "..." menu's Commit entries. An amend may leave the message
    /// empty to keep the last commit's.
    pub(super) fn commit(
        &mut self,
        scope: CommitScope,
        amend: bool,
        signoff: bool,
    ) -> Vec<Command> {
        let message = self.source_control.view.message().to_owned();
        if message.trim().is_empty() && !amend {
            self.message = Some("Type a commit message first.".into());
            self.focus_source_control();
            return Vec::new();
        }
        let staged = self.status.staged > 0;
        let all = match scope {
            CommitScope::Staged => false,
            CommitScope::All => true,
            CommitScope::Auto => !staged,
        };
        if !amend && !all && !staged {
            self.message = Some("Nothing staged: stage files with + first.".into());
            return Vec::new();
        }
        if !amend && all && self.status.is_clean() {
            self.message = Some("Nothing to commit.".into());
            return Vec::new();
        }
        let label = if amend { "Amend" } else { "Commit" };
        let action = WorkingTreeAction::Commit {
            message,
            all,
            amend,
            signoff,
        };
        self.start_action(Action::WorkingTree(action), label.into())
    }
}
