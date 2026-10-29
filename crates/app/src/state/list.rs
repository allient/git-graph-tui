//! A scrollable list drawn over the graph (the reflog and worktrees views):
//! entries, the selected one, the scroll offset and where the rows were drawn.

use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};

use super::SCROLL_LINES;

/// The open list's entries, the selected one, the scroll offset, and where
/// the last frame drew the rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListPanel<T> {
    pub entries: Vec<T>,
    pub selected: usize,
    pub offset: usize,
    /// The rows' area in the last frame (inside the border).
    pub area: Rect,
}

impl<T> Default for ListPanel<T> {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl<T> ListPanel<T> {
    pub fn new(entries: Vec<T>) -> Self {
        Self {
            entries,
            selected: 0,
            offset: 0,
            area: Rect::default(),
        }
    }

    pub fn selected_entry(&self) -> Option<&T> {
        self.entries.get(self.selected)
    }

    fn page(&self) -> usize {
        usize::from(self.area.height).max(1)
    }

    /// Selects an entry (clamped) and scrolls it into view.
    pub fn select(&mut self, index: usize) {
        self.selected = index.min(self.entries.len().saturating_sub(1));
        let page = self.page();
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + page {
            self.offset = self.selected + 1 - page;
        }
    }

    /// Replaces the entries, keeping `kept` selected when given (else the
    /// same position).
    pub fn reload(&mut self, entries: Vec<T>, kept: Option<usize>) {
        self.entries = entries;
        self.select(kept.unwrap_or(self.selected));
    }

    pub fn scroll(&mut self, delta: isize) {
        let max = self.entries.len().saturating_sub(self.page());
        self.offset = self.offset.saturating_add_signed(delta).min(max);
    }

    /// Moves the selection for the list keys (arrows, vim keys, paging);
    /// false when `code` isn't one of them.
    pub fn navigate(&mut self, code: KeyCode) -> bool {
        let page = self.page();
        match code {
            KeyCode::Down | KeyCode::Char('j') => self.select(self.selected + 1),
            KeyCode::Up | KeyCode::Char('k') => self.select(self.selected.saturating_sub(1)),
            KeyCode::PageDown => self.select(self.selected + page),
            KeyCode::PageUp => self.select(self.selected.saturating_sub(page)),
            KeyCode::Home | KeyCode::Char('g') => self.select(0),
            KeyCode::End | KeyCode::Char('G') => self.select(usize::MAX),
            _ => return false,
        }
        true
    }

    /// The entry drawn at (`x`, `y`), if any.
    pub fn entry_at(&self, x: u16, y: u16) -> Option<usize> {
        if !self.area.contains(Position { x, y }) {
            return None;
        }
        let index = self.offset + usize::from(y - self.area.y);
        (index < self.entries.len()).then_some(index)
    }

    /// The wheel scrolls; a click selects the entry under it. True for a
    /// right click on an entry, which opens its menu.
    pub fn on_mouse(&mut self, mouse: MouseEvent) -> bool {
        let lines = isize::try_from(SCROLL_LINES).unwrap_or(isize::MAX);
        match mouse.kind {
            MouseEventKind::ScrollDown => self.scroll(lines),
            MouseEventKind::ScrollUp => self.scroll(-lines),
            MouseEventKind::Down(button @ (MouseButton::Left | MouseButton::Right)) => {
                if let Some(index) = self.entry_at(mouse.column, mouse.row) {
                    self.select(index);
                    return button == MouseButton::Right;
                }
            }
            _ => {}
        }
        false
    }

    /// Where the selected entry's menu opens: just below its row.
    pub fn menu_anchor(&self) -> Position {
        let row = self.selected.saturating_sub(self.offset);
        let y = self
            .area
            .y
            .saturating_add(u16::try_from(row).unwrap_or(u16::MAX))
            .saturating_add(1);
        Position {
            x: self.area.x + 2,
            y,
        }
    }
}
