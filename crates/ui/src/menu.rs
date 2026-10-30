use ggt_actions::{Action, MenuEntry, MenuItem};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Clear, Widget};
use unicode_width::UnicodeWidthStr;

use crate::{Key, Theme, fit};

/// Columns of padding on each side of a label.
const PADDING: u16 = 1;

/// What a key press or click on the menu means for the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuOutcome {
    /// Still open (the selection may have moved).
    Open,
    /// The user picked this entry: close the menu and run (or ask about) it.
    Chosen(Action),
    /// The user picked a disabled entry; show the reason, keep the menu open.
    Disabled(String),
    /// Escape or a click outside: close without doing anything.
    Closed,
}

/// What is under a screen cell, as of the last render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuHit {
    /// A menu entry (index into [`PopupMenu::items`]), enabled or not.
    Item(usize),
    /// Inside the menu but not on an entry (border, separator).
    Frame,
    Outside,
}

/// Where the menu and its entries were drawn.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MenuHitMap {
    area: Rect,
    items: Vec<(Rect, usize)>,
    /// The open submenu, drawn beside this one.
    child: Option<Box<MenuHitMap>>,
}

impl MenuHitMap {
    #[must_use]
    pub fn hit(&self, x: u16, y: u16) -> MenuHit {
        let point = Position { x, y };
        if let Some((_, index)) = self.items.iter().find(|(rect, _)| rect.contains(point)) {
            MenuHit::Item(*index)
        } else if self.area.contains(point) {
            MenuHit::Frame
        } else {
            MenuHit::Outside
        }
    }

    /// The whole menu box.
    #[must_use]
    pub fn area(&self) -> Rect {
        self.area
    }

    /// The open submenu's hit-map, if one is open.
    #[must_use]
    pub fn child(&self) -> Option<&MenuHitMap> {
        self.child.as_deref()
    }

    /// The cell is inside this menu or any open submenu.
    #[must_use]
    pub fn contains(&self, x: u16, y: u16) -> bool {
        self.area.contains(Position { x, y }) || self.child().is_some_and(|c| c.contains(x, y))
    }
}

/// A context menu (S8, S9): a bordered box of entries and separators opened
/// at an anchor and kept inside the screen.
///
/// The selection only ever lands on enabled entries. Disabled entries are
/// drawn dimmed and report their reason when picked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PopupMenu {
    items: Vec<MenuItem>,
    selected: Option<usize>,
    anchor: Position,
    /// The open submenu (of the selected `MenuItem::Submenu`).
    child: Option<Box<PopupMenu>>,
}

impl PopupMenu {
    /// A menu whose top-left corner goes at `anchor` when there is room: the
    /// mouse position for a right-click, or the cell below the selected row's
    /// description for the keyboard menu key.
    #[must_use]
    pub fn new(items: Vec<MenuItem>, anchor: Position) -> Self {
        let mut menu = Self {
            items,
            selected: None,
            anchor,
            child: None,
        };
        menu.select_first();
        menu
    }

    #[must_use]
    pub fn items(&self) -> &[MenuItem] {
        &self.items
    }

    /// Index into [`PopupMenu::items`] of the highlighted entry.
    #[must_use]
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// Label of the highlighted entry at the deepest open level, e.g. the
    /// entry a `MenuOutcome::Chosen` from a submenu came from.
    #[must_use]
    pub fn selected_label(&self) -> Option<&str> {
        if let Some(label) = self.child.as_ref().and_then(|c| c.selected_label()) {
            return Some(label);
        }
        self.label(self.selected?)
    }

    #[must_use]
    pub fn is_child_open(&self) -> bool {
        self.child.is_some()
    }

    #[must_use]
    pub fn selected_action(&self) -> Option<&Action> {
        self.selected.and_then(|i| self.entry(i)).map(|e| &e.action)
    }

    /// Why the entry at `index` is disabled (`None` if it is enabled or not an entry).
    #[must_use]
    pub fn disabled_reason(&self, index: usize) -> Option<&str> {
        self.entry(index)?.disabled.as_deref()
    }

    pub fn select_first(&mut self) {
        let first = self.enabled().next();
        self.selected = first;
    }

    pub fn select_last(&mut self) {
        let last = self.enabled().next_back();
        self.selected = last;
    }

    /// Moves down to the next enabled entry, wrapping to the top.
    pub fn select_next(&mut self) {
        let Some(current) = self.selected else {
            return self.select_first();
        };
        let next = self
            .enabled()
            .find(|&i| i > current)
            .or_else(|| self.enabled().next());
        self.selected = next;
    }

    /// Moves up to the previous enabled entry, wrapping to the bottom.
    pub fn select_prev(&mut self) {
        let Some(current) = self.selected else {
            return self.select_last();
        };
        let prev = self
            .enabled()
            .rev()
            .find(|&i| i < current)
            .or_else(|| self.enabled().next_back());
        self.selected = prev;
    }

    /// Jumps to the next enabled entry whose label starts with `letter`
    /// (case-insensitive), wrapping. Returns false when none matches.
    pub fn jump_to_letter(&mut self, letter: char) -> bool {
        let matches = |menu: &Self, i: usize| {
            menu.label(i)
                .and_then(|label| label.chars().next())
                .is_some_and(|first| first.to_lowercase().eq(letter.to_lowercase()))
        };
        let after = self.selected.map_or(0, |i| i + 1);
        let found = self
            .enabled()
            .filter(|&i| i >= after)
            .chain(self.enabled().filter(|&i| i < after))
            .find(|&i| matches(self, i));
        if found.is_some() {
            self.selected = found;
        }
        found.is_some()
    }

    /// Arrows, Home/End and accelerator letters move; Enter chooses (or
    /// opens a submenu, like Right); Esc closes. With a submenu open, keys go
    /// to it, and Left or Esc closes just the submenu.
    pub fn handle_key(&mut self, key: Key) -> MenuOutcome {
        if let Some(child) = self.child.as_mut() {
            if matches!(key, Key::Left | Key::Esc) && !child.is_child_open() {
                self.child = None;
                return MenuOutcome::Open;
            }
            return child.handle_key(key);
        }
        if matches!(key, Key::Right | Key::Enter) && self.open_child() {
            return MenuOutcome::Open;
        }
        match key {
            Key::Up | Key::BackTab => self.select_prev(),
            Key::Down | Key::Tab => self.select_next(),
            Key::Home | Key::PageUp => self.select_first(),
            Key::End | Key::PageDown => self.select_last(),
            Key::Char(letter) => {
                self.jump_to_letter(letter);
            }
            Key::Enter => {
                if let Some(action) = self.selected_action() {
                    return MenuOutcome::Chosen(action.clone());
                }
            }
            Key::Esc => return MenuOutcome::Closed,
            Key::Left | Key::Right | Key::Backspace | Key::Delete => {}
        }
        MenuOutcome::Open
    }

    /// A left click at `(x, y)`, resolved against the last render. Clicks in
    /// an open submenu go to it; a click on a submenu row opens it.
    pub fn handle_click(&mut self, hits: &MenuHitMap, x: u16, y: u16) -> MenuOutcome {
        if let (Some(child), Some(child_hits)) = (self.child.as_mut(), hits.child())
            && child_hits.contains(x, y)
        {
            return child.handle_click(child_hits, x, y);
        }
        match hits.hit(x, y) {
            MenuHit::Item(index) if self.is_submenu(index) => {
                self.selected = Some(index);
                self.open_child();
                MenuOutcome::Open
            }
            MenuHit::Item(index) => match self.entry(index) {
                Some(MenuEntry {
                    disabled: Some(why),
                    ..
                }) => MenuOutcome::Disabled(why.clone()),
                Some(entry) => {
                    let action = entry.action.clone();
                    self.selected = Some(index);
                    MenuOutcome::Chosen(action)
                }
                None => MenuOutcome::Open,
            },
            MenuHit::Frame => MenuOutcome::Open,
            MenuHit::Outside => MenuOutcome::Closed,
        }
    }

    /// Mouse movement: highlights the enabled entry under the pointer,
    /// opening a submenu row's child and closing it on any other entry.
    pub fn handle_hover(&mut self, hits: &MenuHitMap, x: u16, y: u16) {
        if let (Some(child), Some(child_hits)) = (self.child.as_mut(), hits.child())
            && child_hits.contains(x, y)
        {
            return child.handle_hover(child_hits, x, y);
        }
        if let MenuHit::Item(index) = hits.hit(x, y)
            && self.is_enabled(index)
        {
            let already_open = self.child.is_some() && self.selected == Some(index);
            self.selected = Some(index);
            if !already_open {
                self.child = None;
                self.open_child();
            }
        }
    }

    /// Opens the selected submenu; false when the selection isn't one.
    fn open_child(&mut self) -> bool {
        let Some(MenuItem::Submenu { items, .. }) = self.selected.and_then(|i| self.items.get(i))
        else {
            return false;
        };
        // Placed beside the parent row when drawn; the anchor is unused.
        self.child = Some(Box::new(Self::new(items.clone(), Position::default())));
        true
    }

    /// Where the menu goes on `screen`: at the anchor, flipped left or up
    /// when it would cross the right or bottom edge, and clamped inside.
    #[must_use]
    pub fn area(&self, screen: Rect) -> Rect {
        let (width, height) = self.size(screen);
        Rect::new(
            place(self.anchor.x, width, screen.x, screen.right()),
            place(self.anchor.y, height, screen.y, screen.bottom()),
            width,
            height,
        )
    }

    /// Box size on `screen`: the widest label plus padding and border.
    fn size(&self, screen: Rect) -> (u16, u16) {
        let label_width = self
            .items
            .iter()
            .filter_map(|item| match item {
                MenuItem::Entry(entry) => Some(entry.label.width()),
                // Room for ` ▸`.
                MenuItem::Submenu { label, .. } => Some(label.width() + 2),
                MenuItem::Separator => None,
            })
            .max()
            .unwrap_or(0);
        let width = u16::try_from(label_width)
            .unwrap_or(u16::MAX)
            .saturating_add(2 * PADDING + 2)
            .min(screen.width);
        let height = u16::try_from(self.items.len())
            .unwrap_or(u16::MAX)
            .saturating_add(2)
            .min(screen.height);
        (width, height)
    }

    /// Draws the menu over whatever is on `screen` and records its hit-map.
    pub fn render(&self, screen: Rect, buf: &mut Buffer, theme: &Theme) -> MenuHitMap {
        self.draw(self.area(screen), screen, buf, theme)
    }

    fn draw(&self, area: Rect, screen: Rect, buf: &mut Buffer, theme: &Theme) -> MenuHitMap {
        let mut hits = MenuHitMap {
            area,
            items: Vec::new(),
            child: None,
        };
        if area.width < 2 || area.height < 2 {
            return hits;
        }
        Clear.render(area, buf);
        let frame = Style::default().fg(theme.muted);
        let inner_width = usize::from(area.width - 2);
        let rows = usize::from(area.height - 2);
        let (left, right) = (area.x, area.right() - 1);

        buf.set_string(left, area.y, border('┌', '─', '┐', inner_width), frame);
        buf.set_string(
            left,
            area.bottom() - 1,
            border('└', '─', '┘', inner_width),
            frame,
        );

        // Scroll so the selection stays on screen when the menu is taller
        // than the terminal.
        let offset = self
            .selected
            .map_or(0, |s| s.saturating_sub(rows.saturating_sub(1)))
            .min(self.items.len().saturating_sub(rows));
        for (row, (index, item)) in self
            .items
            .iter()
            .enumerate()
            .skip(offset)
            .take(rows)
            .enumerate()
        {
            let y = area.y + 1 + u16::try_from(row).unwrap_or(u16::MAX);
            match item {
                MenuItem::Separator => {
                    buf.set_string(left, y, border('├', '─', '┤', inner_width), frame);
                }
                MenuItem::Entry(_) | MenuItem::Submenu { .. } => {
                    let disabled = self.disabled_reason(index).is_some();
                    let style = if disabled {
                        Style::default().fg(theme.muted)
                    } else if self.selected == Some(index) {
                        Style::default().fg(theme.text).bg(theme.selected_bg)
                    } else {
                        Style::default().fg(theme.text)
                    };
                    let pad = " ".repeat(usize::from(PADDING));
                    let room = inner_width.saturating_sub(2 * usize::from(PADDING));
                    let text = match item {
                        MenuItem::Submenu { label, .. } => {
                            format!("{}▸", fit(label, room.saturating_sub(1)))
                        }
                        _ => fit(self.label(index).unwrap_or_default(), room),
                    };
                    buf.set_string(left, y, "│", frame);
                    buf.set_string(left + 1, y, format!("{pad}{text}{pad}"), style);
                    buf.set_string(right, y, "│", frame);
                    hits.items
                        .push((Rect::new(left + 1, y, area.width - 2, 1), index));
                }
            }
        }
        if let Some(child) = &self.child {
            // Beside the submenu row; if it was scrolled away, beside the top.
            let row_y = self
                .selected
                .and_then(|s| hits.items.iter().find(|(_, i)| *i == s))
                .map_or(area.y + 1, |(rect, _)| rect.y);
            let child_area = beside(area, row_y, child.size(screen), screen);
            hits.child = Some(Box::new(child.draw(child_area, screen, buf, theme)));
        }
        hits
    }

    fn label(&self, index: usize) -> Option<&str> {
        match self.items.get(index)? {
            MenuItem::Entry(entry) => Some(&entry.label),
            MenuItem::Submenu { label, .. } => Some(label),
            MenuItem::Separator => None,
        }
    }

    fn is_submenu(&self, index: usize) -> bool {
        matches!(self.items.get(index), Some(MenuItem::Submenu { .. }))
    }

    fn entry(&self, index: usize) -> Option<&MenuEntry> {
        match self.items.get(index)? {
            MenuItem::Entry(entry) => Some(entry),
            MenuItem::Separator | MenuItem::Submenu { .. } => None,
        }
    }

    fn is_enabled(&self, index: usize) -> bool {
        self.is_submenu(index) || self.entry(index).is_some_and(|e| e.disabled.is_none())
    }

    fn enabled(&self) -> impl DoubleEndedIterator<Item = usize> + '_ {
        (0..self.items.len()).filter(|&i| self.is_enabled(i))
    }
}

/// A submenu of `size` beside `parent`, its first entry on `row_y`: to the
/// right when it fits, otherwise to the left, then clamped to `screen`.
fn beside(parent: Rect, row_y: u16, (width, height): (u16, u16), screen: Rect) -> Rect {
    let x = if parent.right().saturating_add(width) <= screen.right() {
        parent.right()
    } else {
        parent.x.saturating_sub(width)
    };
    let x = x.clamp(screen.x, screen.right().saturating_sub(width).max(screen.x));
    // The child's top border sits one row above its first entry.
    let y = place(row_y.saturating_sub(1), height, screen.y, screen.bottom());
    Rect::new(x, y, width, height)
}

/// Start of a span of `size` cells anchored at `anchor` within `[low, high)`:
/// at the anchor if it fits, otherwise ending on the anchor, then clamped.
fn place(anchor: u16, size: u16, low: u16, high: u16) -> u16 {
    let start = if anchor.saturating_add(size) <= high {
        anchor
    } else {
        (anchor + 1).saturating_sub(size)
    };
    start.clamp(low, high.saturating_sub(size).max(low))
}

fn border(left: char, fill: char, right: char, inner_width: usize) -> String {
    let mut line = String::with_capacity(inner_width + 2);
    line.push(left);
    line.extend(std::iter::repeat_n(fill, inner_width));
    line.push(right);
    line
}

#[cfg(test)]
mod tests;
