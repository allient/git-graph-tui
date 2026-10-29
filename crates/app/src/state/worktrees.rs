//! "Worktrees..." (S20 4): the repository's working trees, with a menu per
//! entry to copy its path, lock, remove, add or prune worktrees.

use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use tgg_actions::{Action, WorktreeAction, worktree_menu};
use tgg_git::worktree::{self, Worktree};
use tgg_ui::PopupMenu;

use super::list::ListPanel;
use super::{AppState, Command, Request};

/// One row of the worktrees view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeEntry {
    pub worktree: Worktree,
    /// The worktree this app runs in.
    pub is_current: bool,
}

/// The open worktrees view.
pub type WorktreesView = ListPanel<WorktreeEntry>;

impl AppState {
    /// Asks git for the worktrees; the view opens (or reloads) when they arrive.
    pub(super) fn show_worktrees(&mut self) -> Vec<Command> {
        if self.worktrees.is_none() {
            self.message = Some("Loading the worktrees…".into());
        }
        vec![Command::Git(Request::Worktrees)]
    }

    /// Opens the view, or reloads it keeping the selected worktree when it
    /// is still there.
    pub(super) fn on_worktrees_read(&mut self, stdout: &str) {
        let worktrees = match worktree::parse(stdout) {
            Ok(worktrees) => worktrees,
            Err(err) => {
                self.message = Some(format!("Could not read the worktrees: {err}"));
                return;
            }
        };
        let entries: Vec<WorktreeEntry> = worktrees
            .into_iter()
            .map(|worktree| WorktreeEntry {
                is_current: same_folder(Path::new(&worktree.path), &self.repo),
                worktree,
            })
            .collect();
        let Some(view) = &mut self.worktrees else {
            self.message = None;
            self.worktrees = Some(WorktreesView::new(entries));
            return;
        };
        let kept = view.selected_entry().and_then(|old| {
            entries
                .iter()
                .position(|e| e.worktree.path == old.worktree.path)
        });
        view.reload(entries, kept);
    }

    pub(super) fn on_worktrees_key(&mut self, key: KeyEvent) -> Vec<Command> {
        let Some(view) = &mut self.worktrees else {
            return Vec::new();
        };
        if view.navigate(key.code) {
            return Vec::new();
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.worktrees = None,
            KeyCode::Enter | KeyCode::Char('m') | KeyCode::Menu => self.open_worktree_menu(),
            KeyCode::Char('a') => {
                return self.start_action(
                    Action::Worktree(WorktreeAction::add()),
                    "Add Worktree".into(),
                );
            }
            _ => {}
        }
        Vec::new()
    }

    pub(super) fn on_worktrees_mouse(&mut self, mouse: MouseEvent) -> Vec<Command> {
        let Some(view) = &mut self.worktrees else {
            return Vec::new();
        };
        if view.on_mouse(mouse) {
            self.open_worktree_menu();
        }
        Vec::new()
    }

    /// The selected worktree's menu, just below its row.
    fn open_worktree_menu(&mut self) {
        let Some(view) = &self.worktrees else {
            return;
        };
        let Some(entry) = view.selected_entry() else {
            return;
        };
        let items = worktree_menu(&entry.worktree, entry.is_current);
        self.menu = Some(PopupMenu::new(items, view.menu_anchor()));
        self.menu_hits = None;
    }
}

/// Git prints worktree paths with symlinks resolved (`/private/var` on
/// macOS), which the app's own path may not be.
fn same_folder(a: &Path, b: &Path) -> bool {
    a == b
        || match (a.canonicalize(), b.canonicalize()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
}
