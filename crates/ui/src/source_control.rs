//! The Source Control pane: a commit message, a Commit button, and the
//! changed files split into "Staged Changes" and "Changes", like VS Code.

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use tgg_model::{Change, ChangedFile};

use crate::input_dialog::window;
use crate::text::fit;
use crate::{Key, TextField, Theme};

/// Lines above the file list: title, message, Commit button, blank.
const TOP_LINES: u16 = 4;

/// What a key or click asks the app to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceControlOutcome {
    Stay,
    /// Give the keyboard back to the graph.
    Leave,
    Commit(String),
    Stage(Vec<String>),
    Unstage(Vec<String>),
}

/// Where the keyboard is inside the pane.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Focus {
    #[default]
    Message,
    Commit,
    Files,
}

/// One line of the file list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    StagedHeader,
    Staged(usize),
    ChangesHeader,
    Unstaged(usize),
}

/// What is under a cell of the pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hit {
    Message,
    Commit,
    /// An item of the list.
    Row(usize),
    /// The `+` or `−` button of an item.
    Toggle(usize),
}

/// The pane's regions from the last frame.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceControlHitMap {
    regions: Vec<(Rect, Hit)>,
    cursor: Option<Position>,
    area: Rect,
}

impl SourceControlHitMap {
    /// Where the terminal cursor goes while the message is being typed.
    #[must_use]
    pub fn cursor(&self) -> Option<Position> {
        self.cursor
    }

    /// Whether a cell is inside the pane.
    #[must_use]
    pub fn contains(&self, x: u16, y: u16) -> bool {
        self.area.contains(Position { x, y })
    }

    fn hit(&self, x: u16, y: u16) -> Option<Hit> {
        self.regions
            .iter()
            .rev()
            .find(|(area, _)| area.contains(Position { x, y }))
            .map(|(_, hit)| *hit)
    }
}

/// The Source Control pane's state.
#[derive(Debug, Clone, Default)]
pub struct SourceControl {
    message: TextField,
    changes: Vec<ChangedFile>,
    branch: Option<String>,
    focus: Focus,
    /// Selected item while the file list has the keyboard.
    selected: usize,
    /// First item shown.
    offset: usize,
}

impl SourceControl {
    /// Replaces the listed files (after every `git status`).
    pub fn set_changes(&mut self, changes: Vec<ChangedFile>, branch: Option<String>) {
        self.changes = changes;
        self.branch = branch;
        self.selected = self.selected.min(self.items().len().saturating_sub(1));
    }

    #[must_use]
    pub fn message(&self) -> &str {
        self.message.text()
    }

    /// Empties the message, after a commit.
    pub fn clear_message(&mut self) {
        self.message = TextField::default();
    }

    /// Puts the keyboard in the message box.
    pub fn focus_message(&mut self) {
        self.focus = Focus::Message;
    }

    fn items(&self) -> Vec<Item> {
        let mut items = Vec::new();
        let staged: Vec<usize> = (0..self.changes.len())
            .filter(|&i| self.changes[i].staged.is_some())
            .collect();
        let unstaged: Vec<usize> = (0..self.changes.len())
            .filter(|&i| self.changes[i].unstaged.is_some())
            .collect();
        if !staged.is_empty() {
            items.push(Item::StagedHeader);
            items.extend(staged.into_iter().map(Item::Staged));
        }
        if !unstaged.is_empty() {
            items.push(Item::ChangesHeader);
            items.extend(unstaged.into_iter().map(Item::Unstaged));
        }
        items
    }

    pub fn handle_key(&mut self, key: Key) -> SourceControlOutcome {
        match self.focus {
            Focus::Message => match key {
                Key::Enter => return self.commit(),
                Key::Down | Key::Tab => self.focus = Focus::Commit,
                Key::Esc => return SourceControlOutcome::Leave,
                Key::Char(ch) => self.message.insert(ch),
                Key::Backspace => self.message.backspace(),
                Key::Delete => self.message.delete(),
                Key::Left => self.message.left(),
                Key::Right => self.message.right(),
                Key::Home => self.message.home(),
                Key::End => self.message.end(),
                _ => {}
            },
            Focus::Commit => match key {
                Key::Enter | Key::Char(' ') => return self.commit(),
                Key::Up | Key::BackTab => self.focus = Focus::Message,
                Key::Down | Key::Tab if !self.items().is_empty() => self.focus = Focus::Files,
                Key::Esc => return SourceControlOutcome::Leave,
                _ => {}
            },
            Focus::Files => {
                let last = self.items().len().saturating_sub(1);
                match key {
                    Key::Up if self.selected == 0 => self.focus = Focus::Commit,
                    Key::Up => self.selected -= 1,
                    Key::Down => self.selected = (self.selected + 1).min(last),
                    Key::Home => self.selected = 0,
                    Key::End => self.selected = last,
                    Key::Enter | Key::Char(' ') => return self.toggle(self.selected),
                    Key::Char('+' | 's') => return self.only(self.selected, true),
                    Key::Char('-' | 'u') => return self.only(self.selected, false),
                    Key::BackTab => self.focus = Focus::Commit,
                    Key::Esc | Key::Tab => return SourceControlOutcome::Leave,
                    _ => {}
                }
            }
        }
        SourceControlOutcome::Stay
    }

    pub fn handle_click(
        &mut self,
        hits: &SourceControlHitMap,
        x: u16,
        y: u16,
    ) -> SourceControlOutcome {
        match hits.hit(x, y) {
            Some(Hit::Message) => self.focus = Focus::Message,
            Some(Hit::Commit) => {
                self.focus = Focus::Commit;
                return self.commit();
            }
            Some(Hit::Row(item)) => {
                self.focus = Focus::Files;
                self.selected = item;
            }
            Some(Hit::Toggle(item)) => {
                self.focus = Focus::Files;
                self.selected = item;
                return self.toggle(item);
            }
            None => {}
        }
        SourceControlOutcome::Stay
    }

    fn commit(&self) -> SourceControlOutcome {
        SourceControlOutcome::Commit(self.message.text().to_owned())
    }

    /// Stages an unstaged item, unstages a staged one.
    fn toggle(&self, item: usize) -> SourceControlOutcome {
        match self.items().get(item) {
            Some(Item::StagedHeader | Item::Staged(_)) => self.only(item, false),
            Some(Item::ChangesHeader | Item::Unstaged(_)) => self.only(item, true),
            None => SourceControlOutcome::Stay,
        }
    }

    /// Stages (`stage`) or unstages the files of an item; Stay when the item
    /// is in the other section.
    fn only(&self, item: usize, stage: bool) -> SourceControlOutcome {
        let items = self.items();
        let files: Vec<usize> = match (items.get(item), stage) {
            (Some(Item::Unstaged(i)), true) | (Some(Item::Staged(i)), false) => vec![*i],
            (Some(Item::ChangesHeader), true) => items
                .iter()
                .filter_map(|it| match it {
                    Item::Unstaged(i) => Some(*i),
                    _ => None,
                })
                .collect(),
            (Some(Item::StagedHeader), false) => items
                .iter()
                .filter_map(|it| match it {
                    Item::Staged(i) => Some(*i),
                    _ => None,
                })
                .collect(),
            _ => return SourceControlOutcome::Stay,
        };
        if stage {
            let paths = files
                .iter()
                .map(|&i| self.changes[i].path.clone())
                .collect();
            SourceControlOutcome::Stage(paths)
        } else {
            // A staged rename unstages both its new and its old path.
            let paths = files
                .iter()
                .flat_map(|&i| {
                    let file = &self.changes[i];
                    std::iter::once(file.path.clone()).chain(file.orig_path.clone())
                })
                .collect();
            SourceControlOutcome::Unstage(paths)
        }
    }

    /// Draws the pane; `focused` when it has the keyboard.
    pub fn render(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        focused: bool,
    ) -> SourceControlHitMap {
        let mut hits = SourceControlHitMap {
            area,
            ..SourceControlHitMap::default()
        };
        if area.width < 4 || area.height == 0 {
            return hits;
        }
        let width = usize::from(area.width);
        let line = |y: u16| Rect::new(area.x, area.y + y, area.width, 1);

        // Title.
        let title = Style::new()
            .fg(theme.text)
            .bg(theme.header_bg)
            .add_modifier(Modifier::BOLD);
        buf.set_style(line(0), title);
        buf.set_stringn(area.x, area.y, " SOURCE CONTROL", width, title);
        if area.height < TOP_LINES {
            return hits;
        }

        // Message box.
        let input = line(1);
        let text_x = area.x + 1;
        let room = width - 1;
        buf.set_style(input, Style::new().bg(theme.selected_bg));
        if self.message.text().is_empty() {
            let branch = self
                .branch
                .as_deref()
                .map_or_else(String::new, |b| format!(" on \"{b}\""));
            let hint = fit(&format!("Message (Enter to commit{branch})"), room);
            buf.set_stringn(
                text_x,
                input.y,
                hint.trim_end(),
                room,
                Style::new().fg(theme.muted),
            );
        } else {
            let (visible, cursor) = window(&self.message, room);
            buf.set_stringn(text_x, input.y, visible, room, Style::new().fg(theme.text));
            if focused && self.focus == Focus::Message {
                hits.cursor = Some(Position::new(text_x + cursor, input.y));
            }
        }
        if focused && self.focus == Focus::Message && self.message.text().is_empty() {
            hits.cursor = Some(Position::new(text_x, input.y));
        }
        hits.regions.push((input, Hit::Message));

        // Commit button, across the pane but the margins.
        let button = Rect::new(area.x + 1, area.y + 2, area.width - 2, 1);
        let mut style = Style::new().fg(theme.badge_fg).bg(theme.lane(0));
        if focused && self.focus == Focus::Commit {
            style = style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
        }
        buf.set_style(button, style);
        let label = "✓ Commit";
        let x = button.x + (button.width.saturating_sub(8)) / 2;
        buf.set_stringn(x, button.y, label, usize::from(button.width), style);
        hits.regions.push((button, Hit::Commit));

        // File list, scrolled so the selection stays visible.
        let items = self.items();
        let rows = usize::from(area.height - TOP_LINES);
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if rows > 0 && self.selected >= self.offset + rows {
            self.offset = self.selected + 1 - rows;
        }
        for (row, (index, item)) in items
            .iter()
            .enumerate()
            .skip(self.offset)
            .take(rows)
            .enumerate()
        {
            let y = area.y + TOP_LINES + u16::try_from(row).unwrap_or(u16::MAX);
            let row_area = Rect::new(area.x, y, area.width, 1);
            if focused && self.focus == Focus::Files && index == self.selected {
                buf.set_style(row_area, Style::new().bg(theme.selected_bg));
            }
            self.draw_item(buf, row_area, *item, &items, theme);
            hits.regions.push((row_area, Hit::Row(index)));
            hits.regions
                .push((Rect::new(row_area.right() - 1, y, 1, 1), Hit::Toggle(index)));
        }
        hits
    }

    fn draw_item(&self, buf: &mut Buffer, area: Rect, item: Item, items: &[Item], theme: &Theme) {
        let text = Style::new().fg(theme.text);
        let muted = Style::new().fg(theme.muted);
        let right = area.right();
        let (left, letter, button) = match item {
            Item::StagedHeader | Item::ChangesHeader => {
                let staged = item == Item::StagedHeader;
                let count = items
                    .iter()
                    .filter(|it| {
                        matches!(
                            (it, staged),
                            (Item::Staged(_), true) | (Item::Unstaged(_), false)
                        )
                    })
                    .count();
                let title = if staged {
                    "▾ Staged Changes"
                } else {
                    "▾ Changes"
                };
                buf.set_stringn(
                    area.x,
                    area.y,
                    title,
                    usize::from(area.width),
                    text.add_modifier(Modifier::BOLD),
                );
                (
                    None,
                    Some((count.to_string(), muted)),
                    if staged { '−' } else { '+' },
                )
            }
            Item::Staged(i) | Item::Unstaged(i) => {
                let file = &self.changes[i];
                let change = if matches!(item, Item::Staged(_)) {
                    file.staged
                } else {
                    file.unstaged
                };
                let (name, dir) = match file.path.rsplit_once('/') {
                    Some((dir, name)) => (name, dir),
                    None => (file.path.as_str(), ""),
                };
                let letter = change.map(|c| {
                    (
                        c.letter().to_string(),
                        Style::new().fg(change_color(c, theme)),
                    )
                });
                (
                    Some((name, dir)),
                    letter,
                    if matches!(item, Item::Staged(_)) {
                        '−'
                    } else {
                        '+'
                    },
                )
            }
        };
        // Right side: `<letter or count> <button>`.
        let tail = 4;
        if let Some((name, dir)) = left {
            let room = usize::from(area.width).saturating_sub(2 + tail);
            let mut x = area.x + 2;
            let name_fit = fit(name, room.min(unicode_width::UnicodeWidthStr::width(name)));
            x = buf.set_stringn(x, area.y, &name_fit, room, text).0;
            let used = usize::from(x - area.x - 2);
            if !dir.is_empty() && used + 1 < room {
                buf.set_stringn(x + 1, area.y, dir, room - used - 1, muted);
            }
        }
        if let Some((label, style)) = letter {
            let width = u16::try_from(label.len()).unwrap_or(u16::MAX);
            buf.set_string(right.saturating_sub(2 + width), area.y, label, style);
        }
        buf[(right - 1, area.y)].set_char(button).set_fg(theme.text);
    }
}

fn change_color(change: Change, theme: &Theme) -> Color {
    match change {
        Change::Modified | Change::TypeChanged => theme.tag,
        Change::Added | Change::Untracked => theme.lane(2),
        Change::Deleted | Change::Conflicted => theme.lane(5),
        Change::Renamed | Change::Copied => theme.lane(4),
    }
}

#[cfg(test)]
mod tests;
