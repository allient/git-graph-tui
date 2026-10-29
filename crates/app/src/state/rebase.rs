//! "Interactive Rebase from Here..." (S20 4): a panel over the graph that
//! lists the commits after a base, oldest first like git's todo list, where
//! each one is picked, squashed, fixed up or dropped and can be moved, then
//! runs the rebase after a confirmation.

use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};
use tgg_actions::{Action, Direction, InteractiveRebase, TodoAction};
use tgg_model::{Commit, Oid};

use super::{AppState, Command, SCROLL_LINES};

/// Shown when the working tree has changes a rebase could clash with.
const DIRTY: &str = "Commit or stash your changes first.";

/// The open interactive rebase panel: the list being edited, the selected
/// commit, the scroll offset, and where the last frame drew the rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebaseView {
    pub rebase: InteractiveRebase,
    pub selected: usize,
    pub offset: usize,
    /// The rows' area in the last frame (inside the border, above the hints).
    pub area: Rect,
}

impl RebaseView {
    fn page(&self) -> usize {
        usize::from(self.area.height).max(1)
    }

    /// Selects a commit (clamped) and scrolls it into view.
    fn select(&mut self, index: usize) {
        self.selected = index.min(self.rebase.items.len().saturating_sub(1));
        let page = self.page();
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + page {
            self.offset = self.selected + 1 - page;
        }
    }

    fn scroll(&mut self, delta: isize) {
        let max = self.rebase.items.len().saturating_sub(self.page());
        self.offset = self.offset.saturating_add_signed(delta).min(max);
    }

    /// The commit drawn on screen row `y`, if any.
    fn item_at(&self, x: u16, y: u16) -> Option<usize> {
        if !self.area.contains(Position { x, y }) {
            return None;
        }
        let index = self.offset + usize::from(y - self.area.y);
        (index < self.rebase.items.len()).then_some(index)
    }

    /// Moves the selected commit and keeps it selected.
    fn move_selected(&mut self, direction: Direction) {
        let to = self.rebase.move_item(self.selected, direction);
        self.select(to);
    }
}

impl AppState {
    /// The commits HEAD reaches through first parents before `base`, newest
    /// first, when none of them is a merge. None when `base` isn't on that
    /// line (or isn't loaded), or a merge commit is in the way.
    pub(super) fn linear_range(&self, base: &Oid) -> Option<Vec<&Commit>> {
        let by_id: HashMap<&Oid, &Commit> = self.commits.iter().map(|c| (&c.id, c)).collect();
        let mut range = Vec::new();
        let mut at = self.head.as_ref()?;
        while at != base {
            let commit = by_id.get(at)?;
            if commit.parents.len() != 1 {
                return None;
            }
            range.push(*commit);
            at = &commit.parents[0];
        }
        Some(range)
    }

    /// Staged, unstaged or conflicted changes: a rebase would refuse or
    /// clash with them. Untracked files are left alone by git.
    fn has_tracked_changes(&self) -> bool {
        self.status.staged + self.status.unstaged + self.status.conflicted > 0
    }

    /// Opens the panel for the commits after `base`, all picked.
    pub(super) fn open_interactive_rebase(&mut self, base: Oid) {
        if self.has_tracked_changes() {
            self.message = Some(DIRTY.into());
            return;
        }
        let Some(branch) = self.menu_context().current_branch().map(str::to_owned) else {
            self.message = Some("Interactive rebase needs HEAD on a branch.".into());
            return;
        };
        let Some(range) = self.linear_range(&base) else {
            self.message =
                Some("Only a straight line of commits after it, up to HEAD, can be edited.".into());
            return;
        };
        let commits = range
            .into_iter()
            .rev()
            .map(|c| (c.id.clone(), c.subject.clone()))
            .collect();
        self.message = None;
        self.rebase = Some(RebaseView {
            rebase: InteractiveRebase::new(base, branch, commits),
            selected: 0,
            offset: 0,
            area: Rect::default(),
        });
    }

    pub(super) fn on_rebase_key(&mut self, key: KeyEvent) -> Vec<Command> {
        if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
            self.rebase = None;
            self.message = Some("Interactive rebase cancelled: nothing changed.".into());
            return Vec::new();
        }
        let Some(view) = &mut self.rebase else {
            return Vec::new();
        };
        let page = view.page();
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let mut set = None;
        match key.code {
            KeyCode::Down if alt => view.move_selected(Direction::Down),
            KeyCode::Up if alt => view.move_selected(Direction::Up),
            KeyCode::Char('J') => view.move_selected(Direction::Down),
            KeyCode::Char('K') => view.move_selected(Direction::Up),
            KeyCode::Down | KeyCode::Char('j') => view.select(view.selected + 1),
            KeyCode::Up | KeyCode::Char('k') => view.select(view.selected.saturating_sub(1)),
            KeyCode::PageDown => view.select(view.selected + page),
            KeyCode::PageUp => view.select(view.selected.saturating_sub(page)),
            KeyCode::Home | KeyCode::Char('g') => view.select(0),
            KeyCode::End | KeyCode::Char('G') => view.select(usize::MAX),
            KeyCode::Char('p') => set = Some(TodoAction::Pick),
            KeyCode::Char('s') => set = Some(TodoAction::Squash),
            KeyCode::Char('f') => set = Some(TodoAction::Fixup),
            KeyCode::Char('d') => set = Some(TodoAction::Drop),
            KeyCode::Char(' ') => view.rebase.cycle(view.selected),
            KeyCode::Enter => return self.start_interactive_rebase(),
            _ => {}
        }
        if let Some(action) = set
            && let Err(err) = view.rebase.set_action(view.selected, action)
        {
            self.message = Some(err.to_string());
        }
        Vec::new()
    }

    pub(super) fn on_rebase_mouse(&mut self, mouse: MouseEvent) -> Vec<Command> {
        let Some(view) = &mut self.rebase else {
            return Vec::new();
        };
        let lines = isize::try_from(SCROLL_LINES).unwrap_or(isize::MAX);
        match mouse.kind {
            MouseEventKind::ScrollDown => view.scroll(lines),
            MouseEventKind::ScrollUp => view.scroll(-lines),
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(index) = view.item_at(mouse.column, mouse.row) {
                    view.select(index);
                }
            }
            _ => {}
        }
        Vec::new()
    }

    /// Enter: checks the list and the working tree, then asks to confirm.
    /// The panel stays open until the rebase runs, so a cancelled
    /// confirmation keeps the edits.
    fn start_interactive_rebase(&mut self) -> Vec<Command> {
        let Some(view) = &self.rebase else {
            return Vec::new();
        };
        if let Err(err) = view.rebase.validate() {
            self.message = Some(err.to_string());
            return Vec::new();
        }
        if self.has_tracked_changes() {
            self.message = Some(DIRTY.into());
            return Vec::new();
        }
        let action = Action::InteractiveRebase(view.rebase.clone());
        self.start_action(action, "Interactive Rebase".into())
    }
}
