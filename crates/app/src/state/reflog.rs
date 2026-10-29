//! "Show Reflog" (S20 4): where HEAD has been, newest first, and a menu
//! per entry to get its commit back (a branch, a checkout or a reset).

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};
use tgg_actions::reflog_menu;
use tgg_git::reflog::{self, ReflogEntry};
use tgg_ui::PopupMenu;

use super::{AppState, Command, Request, SCROLL_LINES};

/// The open reflog view: its entries, the selected one, the scroll offset,
/// and where the last frame drew the rows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReflogView {
    pub entries: Vec<ReflogEntry>,
    pub selected: usize,
    pub offset: usize,
    /// The rows' area in the last frame (inside the border).
    pub area: Rect,
}

impl ReflogView {
    fn page(&self) -> usize {
        usize::from(self.area.height).max(1)
    }

    /// Selects an entry (clamped) and scrolls it into view.
    fn select(&mut self, index: usize) {
        self.selected = index.min(self.entries.len().saturating_sub(1));
        let page = self.page();
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + page {
            self.offset = self.selected + 1 - page;
        }
    }

    fn scroll(&mut self, delta: isize) {
        let max = self.entries.len().saturating_sub(self.page());
        self.offset = self.offset.saturating_add_signed(delta).min(max);
    }

    /// The entry drawn on screen row `y`, if any.
    fn entry_at(&self, x: u16, y: u16) -> Option<usize> {
        if !self.area.contains(Position { x, y }) {
            return None;
        }
        let index = self.offset + usize::from(y - self.area.y);
        (index < self.entries.len()).then_some(index)
    }
}

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
            self.reflog = Some(ReflogView {
                entries,
                ..ReflogView::default()
            });
            return;
        };
        let kept = view.entries.get(view.selected).and_then(|old| {
            entries
                .iter()
                .position(|e| e.id == old.id && e.time == old.time && e.subject == old.subject)
        });
        view.entries = entries;
        view.select(kept.unwrap_or(view.selected));
    }

    pub(super) fn on_reflog_key(&mut self, key: KeyEvent) -> Vec<Command> {
        let Some(view) = &mut self.reflog else {
            return Vec::new();
        };
        let page = view.page();
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.reflog = None,
            KeyCode::Down | KeyCode::Char('j') => view.select(view.selected + 1),
            KeyCode::Up | KeyCode::Char('k') => view.select(view.selected.saturating_sub(1)),
            KeyCode::PageDown => view.select(view.selected + page),
            KeyCode::PageUp => view.select(view.selected.saturating_sub(page)),
            KeyCode::Home | KeyCode::Char('g') => view.select(0),
            KeyCode::End | KeyCode::Char('G') => view.select(usize::MAX),
            KeyCode::Enter | KeyCode::Char('m') | KeyCode::Menu => self.open_reflog_menu(),
            _ => {}
        }
        Vec::new()
    }

    pub(super) fn on_reflog_mouse(&mut self, mouse: MouseEvent) -> Vec<Command> {
        let Some(view) = &mut self.reflog else {
            return Vec::new();
        };
        let lines = isize::try_from(SCROLL_LINES).unwrap_or(isize::MAX);
        match mouse.kind {
            MouseEventKind::ScrollDown => view.scroll(lines),
            MouseEventKind::ScrollUp => view.scroll(-lines),
            MouseEventKind::Down(button @ (MouseButton::Left | MouseButton::Right)) => {
                if let Some(index) = view.entry_at(mouse.column, mouse.row) {
                    view.select(index);
                    if button == MouseButton::Right {
                        self.open_reflog_menu();
                    }
                }
            }
            _ => {}
        }
        Vec::new()
    }

    /// The recovery menu of the selected entry, just below its row.
    fn open_reflog_menu(&mut self) {
        let Some(view) = &self.reflog else {
            return;
        };
        let Some(entry) = view.entries.get(view.selected) else {
            return;
        };
        let row = view.selected.saturating_sub(view.offset);
        let y = view
            .area
            .y
            .saturating_add(u16::try_from(row).unwrap_or(u16::MAX))
            .saturating_add(1);
        let anchor = Position {
            x: view.area.x + 2,
            y,
        };
        let items = reflog_menu(&entry.id, &entry.commit_subject, &self.menu_context());
        self.menu = Some(PopupMenu::new(items, anchor));
        self.menu_hits = None;
    }
}
