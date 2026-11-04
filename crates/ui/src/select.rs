//! A dropdown selector: a button showing the chosen option (`mixed ▾`) and,
//! when open, a menu-style list of every option hanging under it (drawn
//! with the same [`crate::menu_list`] as the context menus).
//!
//! The widget owns its state (options, choice, highlight); its host (a
//! dialog, a toolbar) gives it keys and clicks, decides where the button
//! goes, and draws the list last so it lies on top of everything.

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Style};
use unicode_width::UnicodeWidthStr;

use crate::menu_list::{CHROME, ListItem, ListRow, draw_list, row_width};
use crate::{Key, Theme, fit};

/// Columns between an option and its description in the open list.
const DETAIL_GAP: usize = 3;

/// One option: the value the host gets back, and what the list says about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectOption {
    pub value: String,
    /// What the option does; empty for plain values such as remote names.
    pub description: String,
    /// Loses work (a hard reset): its description is in the warning color.
    pub destructive: bool,
}

impl SelectOption {
    /// An option without a description.
    #[must_use]
    pub fn plain(value: &str) -> Self {
        Self {
            value: value.to_owned(),
            description: String::new(),
            destructive: false,
        }
    }
}

/// What a key or click did to the selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectEvent {
    /// Not for the selector: the host handles it (Tab, Enter on a closed
    /// selector, ...).
    Ignored,
    /// Opened, closed or moved the highlight; the value is the same.
    Handled,
    /// The chosen option changed.
    Changed,
}

/// Where the open list and its options were drawn.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SelectHitMap {
    area: Option<Rect>,
    options: Vec<(Rect, usize)>,
}

/// What is under a cell, for the open list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectHit {
    Option(usize),
    /// The list's frame.
    List,
    Outside,
}

impl SelectHitMap {
    #[must_use]
    pub fn hit(&self, x: u16, y: u16) -> SelectHit {
        let point = Position { x, y };
        if let Some((_, i)) = self.options.iter().find(|(r, _)| r.contains(point)) {
            SelectHit::Option(*i)
        } else if self.area.is_some_and(|r| r.contains(point)) {
            SelectHit::List
        } else {
            SelectHit::Outside
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Select {
    options: Vec<SelectOption>,
    chosen: usize,
    /// The highlighted option while the list is open.
    open: Option<usize>,
}

impl Select {
    /// A selector over `options` set to `current` (added first if missing, so
    /// an unexpected value is never lost).
    #[must_use]
    pub fn new(mut options: Vec<SelectOption>, current: &str) -> Self {
        let chosen = options
            .iter()
            .position(|o| o.value == current)
            .unwrap_or_else(|| {
                options.insert(0, SelectOption::plain(current));
                0
            });
        Self {
            options,
            chosen,
            open: None,
        }
    }

    /// The chosen option's value.
    #[must_use]
    pub fn value(&self) -> &str {
        self.options
            .get(self.chosen)
            .map_or("", |o| o.value.as_str())
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Opens the list with the chosen option highlighted.
    pub fn open(&mut self) {
        self.open = Some(self.chosen);
    }

    pub fn close(&mut self) {
        self.open = None;
    }

    /// Closed: Space opens the list, Left/Right switch options without it.
    /// Open: Up/Down/Home/End move, Enter or Space pick, Esc or Tab close;
    /// every key stays with the selector.
    pub fn handle_key(&mut self, key: Key) -> SelectEvent {
        let last = self.options.len().saturating_sub(1);
        let Some(highlighted) = self.open else {
            return match key {
                Key::Char(' ') => {
                    self.open();
                    SelectEvent::Handled
                }
                Key::Right => self.choose((self.chosen + 1) % self.options.len().max(1)),
                Key::Left => self.choose((self.chosen + last) % self.options.len().max(1)),
                _ => SelectEvent::Ignored,
            };
        };
        match key {
            Key::Up => self.open = Some(highlighted.saturating_sub(1)),
            Key::Down => self.open = Some((highlighted + 1).min(last)),
            Key::Home => self.open = Some(0),
            Key::End => self.open = Some(last),
            Key::Enter | Key::Char(' ') => {
                self.open = None;
                return self.choose(highlighted);
            }
            Key::Esc | Key::Tab | Key::BackTab => self.open = None,
            _ => {}
        }
        SelectEvent::Handled
    }

    /// A click while the list is open: an option is picked, anywhere else
    /// only closes the list.
    pub fn handle_click(&mut self, hits: &SelectHitMap, x: u16, y: u16) -> SelectEvent {
        if !self.is_open() {
            return SelectEvent::Ignored;
        }
        self.open = None;
        match hits.hit(x, y) {
            SelectHit::Option(option) => self.choose(option),
            SelectHit::List | SelectHit::Outside => SelectEvent::Handled,
        }
    }

    fn choose(&mut self, option: usize) -> SelectEvent {
        if option >= self.options.len() || option == self.chosen {
            return SelectEvent::Handled;
        }
        self.chosen = option;
        SelectEvent::Changed
    }

    /// The button's width: a space, the longest option, a space, `▾`, a
    /// space. The same whichever option is chosen, like a real select.
    #[must_use]
    pub fn button_width(&self) -> usize {
        self.options
            .iter()
            .map(|o| o.value.width())
            .max()
            .unwrap_or(0)
            + 4
    }

    /// Draws the button at the start of `row` in `style`, and the chosen
    /// option's description beside it.
    pub fn render_button(
        &self,
        buf: &mut Buffer,
        row: Rect,
        style: Style,
        theme: &Theme,
        warning: Color,
    ) {
        let room = usize::from(row.width);
        let button = self.button_width().min(room);
        let arrow = if self.is_open() { "▴" } else { "▾" };
        let label = format!(" {} {arrow} ", fit(self.value(), button.saturating_sub(4)));
        buf.set_string(row.x, row.y, fit(&label, button), style);
        let Some(option) = self
            .options
            .get(self.chosen)
            .filter(|o| !o.description.is_empty())
        else {
            return;
        };
        let color = if option.destructive {
            warning
        } else {
            theme.muted
        };
        buf.set_string(
            row.x + to_u16(button + 2),
            row.y,
            fit(&option.description, room.saturating_sub(button + 2)),
            Style::default().fg(color),
        );
    }

    /// Draws the open list hanging from `button` (the row the button is on),
    /// on `screen`. Draw it after everything else: it lies on top.
    pub fn render_list(
        &self,
        buf: &mut Buffer,
        button: Rect,
        screen: Rect,
        theme: &Theme,
        warning: Color,
    ) -> SelectHitMap {
        let Some(highlighted) = self.open else {
            return SelectHitMap::default();
        };
        let labels: Vec<String> = self
            .options
            .iter()
            .enumerate()
            .map(|(i, o)| {
                let tick = if i == self.chosen { "✓" } else { " " };
                format!("{tick} {}", o.value)
            })
            .collect();
        let widest = self
            .options
            .iter()
            .zip(&labels)
            .map(|(o, label)| row_width(label, &o.description, DETAIL_GAP))
            .max()
            .unwrap_or(0);
        let width = (widest + CHROME).max(self.button_width() + 2);
        let height = to_u16(self.options.len()).saturating_add(2);
        // One column left of the button, so the options line up with it.
        let left = button.x.saturating_sub(1);
        let area = Rect::new(
            left,
            list_top(button, height, screen),
            to_u16(width).min(screen.right().saturating_sub(left)),
            height,
        )
        .intersection(screen);
        let rows: Vec<ListRow<'_>> = self
            .options
            .iter()
            .zip(&labels)
            .enumerate()
            .map(|(i, (o, label))| {
                ListRow::Item(ListItem {
                    label,
                    detail: &o.description,
                    highlighted: i == highlighted,
                    disabled: false,
                    warning: o.destructive,
                })
            })
            .collect();
        let offset = highlighted.saturating_sub(usize::from(area.height.saturating_sub(3)));
        SelectHitMap {
            area: Some(area),
            options: draw_list(buf, area, &rows, offset, theme, warning),
        }
    }
}

/// Where a list `height` rows tall starts: under `button` when it fits, else
/// above it when that fits, else under it, cut off by the screen. Never over
/// the button itself.
fn list_top(button: Rect, height: u16, screen: Rect) -> u16 {
    let below = button.bottom();
    if below.saturating_add(height) <= screen.bottom() {
        below
    } else if button.y >= screen.y.saturating_add(height) {
        button.y - height
    } else {
        below
    }
}

fn to_u16(n: usize) -> u16 {
    u16::try_from(n).unwrap_or(u16::MAX)
}

#[cfg(test)]
mod tests;
