//! "Show Reflog" (S20 4): where HEAD has been, newest first, and a menu
//! per entry to get its commit back (a branch, a checkout or a reset).

use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ggt_actions::reflog_menu;
use ggt_git::reflog::{self, ReflogEntry};
use ggt_ui::PopupMenu;

use super::list::ListPanel;
use super::{AppState, Command, Request};

/// The open reflog view.
pub type ReflogView = ListPanel<ReflogEntry>;

impl AppState {
    /// Asks git for the reflog; the view opens (or reloads) when it arrives.
    pub(super) fn show_reflog(&mut self) -> Vec<Command> {
        if self.reflog.is_none() {
            self.message = Some("Loading the reflog…".into());
        }
        vec![Command::Git(Request::Reflog)]
    }

    /// Opens the view, or reloads it keeping the selected entry when it is
    /// still there (after an action, new entries push it down).
    pub(super) fn on_reflog_read(&mut self, stdout: &str) {
        let entries = match reflog::parse(stdout) {
            Ok(entries) => entries,
            Err(err) => {
                self.message = Some(format!("Could not read the reflog: {err}"));
                return;
            }
        };
        let Some(view) = &mut self.reflog else {
            self.message = None;
            self.reflog = Some(ReflogView::new(entries));
            return;
        };
        let kept = view.selected_entry().and_then(|old| {
            entries
                .iter()
                .position(|e| e.id == old.id && e.time == old.time && e.subject == old.subject)
        });
        view.reload(entries, kept);
    }

    pub(super) fn on_reflog_key(&mut self, key: KeyEvent) -> Vec<Command> {
        let Some(view) = &mut self.reflog else {
            return Vec::new();
        };
        if view.navigate(key.code) {
            return Vec::new();
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.reflog = None,
            KeyCode::Enter | KeyCode::Char('m') | KeyCode::Menu => self.open_reflog_menu(),
            _ => {}
        }
        Vec::new()
    }

    pub(super) fn on_reflog_mouse(&mut self, mouse: MouseEvent) -> Vec<Command> {
        let Some(view) = &mut self.reflog else {
            return Vec::new();
        };
        if view.on_mouse(mouse) {
            self.open_reflog_menu();
        }
        Vec::new()
    }

    /// The recovery menu of the selected entry, just below its row.
    fn open_reflog_menu(&mut self) {
        let Some(view) = &self.reflog else {
            return;
        };
        let Some(entry) = view.selected_entry() else {
            return;
        };
        let items = reflog_menu(&entry.id, &entry.commit_subject, &self.menu_context());
        self.menu = Some(PopupMenu::new(items, view.menu_anchor()));
        self.menu_hits = None;
    }
}
