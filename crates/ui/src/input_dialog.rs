use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use tgg_actions::{Action, InputId, InputKind, InputValue};
use unicode_width::UnicodeWidthChar;

use crate::confirm::{MAX_WIDTH, centered, draw_buttons, draw_frame, text_width};
use crate::{Button, DialogStyle, Key, Theme, fit};

/// A single-line text input. The cursor is a char index, so editing never
/// splits a UTF-8 sequence.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextField {
    text: String,
    cursor: usize,
}

impl TextField {
    /// A field holding `text`, with the cursor at the end.
    #[must_use]
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            cursor: text.chars().count(),
        }
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Position of the cursor, in chars from the start.
    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Moves the cursor to char index `cursor` (clamped to the text).
    pub fn set_cursor(&mut self, cursor: usize) {
        self.cursor = cursor.min(self.text.chars().count());
    }

    /// Display columns before the cursor (wide characters count twice).
    #[must_use]
    pub fn width_before_cursor(&self) -> usize {
        self.text.chars().take(self.cursor).map(width).sum()
    }

    pub fn insert(&mut self, ch: char) {
        let at = self.byte_offset(self.cursor);
        self.text.insert(at, ch);
        self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.remove_at_cursor();
        }
    }

    pub fn delete(&mut self) {
        if self.cursor < self.len() {
            self.remove_at_cursor();
        }
    }

    pub fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.len());
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.len();
    }

    fn len(&self) -> usize {
        self.text.chars().count()
    }

    fn byte_offset(&self, chars: usize) -> usize {
        self.text
            .char_indices()
            .nth(chars)
            .map_or(self.text.len(), |(i, _)| i)
    }

    fn remove_at_cursor(&mut self) {
        let at = self.byte_offset(self.cursor);
        self.text.remove(at);
    }
}

fn width(ch: char) -> usize {
    ch.width().unwrap_or(0)
}

/// What has keyboard focus: a field (index) or a button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Field(usize),
    Button(Button),
}

/// What a key press or click means for the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputOutcome {
    Pending,
    /// Every value passed validation; this is the action with them applied.
    /// Next: its confirmation (if any), then its plan.
    Submitted(Action),
    Cancelled,
}

/// What is under a screen cell, as of the last render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputHit {
    Field(usize),
    Button(Button),
    Frame,
    Outside,
}

/// Where the dialog, its fields and buttons were drawn, and where the
/// terminal cursor belongs (for `Frame::set_cursor_position`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InputHitMap {
    area: Rect,
    fields: Vec<(Rect, usize)>,
    buttons: Vec<(Rect, Button)>,
    cursor: Option<Position>,
}

impl InputHitMap {
    #[must_use]
    pub fn hit(&self, x: u16, y: u16) -> InputHit {
        let point = Position { x, y };
        if let Some((_, i)) = self.fields.iter().find(|(r, _)| r.contains(point)) {
            InputHit::Field(*i)
        } else if let Some((_, b)) = self.buttons.iter().find(|(r, _)| r.contains(point)) {
            InputHit::Button(*b)
        } else if self.area.contains(point) {
            InputHit::Frame
        } else {
            InputHit::Outside
        }
    }

    #[must_use]
    pub fn area(&self) -> Rect {
        self.area
    }

    /// The editable row of field `index` (the text box, toggle or choice).
    #[must_use]
    pub fn field(&self, index: usize) -> Option<Rect> {
        self.fields
            .iter()
            .find(|(_, i)| *i == index)
            .map(|(r, _)| *r)
    }

    #[must_use]
    pub fn button(&self, button: Button) -> Option<Rect> {
        self.buttons
            .iter()
            .find(|(_, b)| *b == button)
            .map(|(r, _)| *r)
    }

    /// The text cursor, when a text field has focus.
    #[must_use]
    pub fn cursor(&self) -> Option<Position> {
        self.cursor
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Value {
    Text(TextField),
    Toggle(bool),
    Choice { options: Vec<String>, index: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Field {
    id: InputId,
    label: &'static str,
    value: Value,
}

impl Field {
    /// Label, value and error rows for text and choices; one row for toggles.
    fn rows(&self) -> u16 {
        match self.value {
            Value::Toggle(_) => 1,
            Value::Text(_) | Value::Choice { .. } => 3,
        }
    }

    fn input_value(&self) -> InputValue {
        match &self.value {
            Value::Text(field) => InputValue::Text(field.text().to_owned()),
            Value::Toggle(on) => InputValue::Bool(*on),
            Value::Choice { options, index } => {
                InputValue::Text(options.get(*index).cloned().unwrap_or_default())
            }
        }
    }
}

/// The dialog that asks for an action's inputs (branch name, tag message,
/// reset mode, ...), validates them and returns the completed action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputDialog {
    title: String,
    action: Action,
    fields: Vec<Field>,
    focus: Focus,
    error: Option<(usize, String)>,
    style: DialogStyle,
}

impl InputDialog {
    /// The dialog for `action`'s inputs, prefilled with its current values,
    /// or `None` when it has none. `title` is also the confirm button label
    /// (e.g. the menu label without `...`).
    #[must_use]
    pub fn for_action(title: &str, action: &Action) -> Option<Self> {
        let fields: Vec<Field> = action
            .inputs()
            .into_iter()
            .map(|spec| {
                let text = match &spec.value {
                    InputValue::Text(text) => text.clone(),
                    InputValue::Bool(_) => String::new(),
                };
                let value = match (spec.kind, &spec.value) {
                    (_, InputValue::Bool(on)) => Value::Toggle(*on),
                    (InputKind::Choice(options), _) => {
                        choice(options.iter().map(|o| (*o).to_owned()), &text)
                    }
                    _ => Value::Text(TextField::new(&text)),
                };
                Field {
                    id: spec.id,
                    label: spec.label,
                    value,
                }
            })
            .collect();
        (!fields.is_empty()).then(|| Self {
            title: title.to_owned(),
            action: action.clone(),
            fields,
            focus: Focus::Field(0),
            error: None,
            style: DialogStyle::default(),
        })
    }

    /// Offers the repository's remotes as a choice instead of free text.
    #[must_use]
    pub fn with_remotes(mut self, remotes: &[String]) -> Self {
        for field in &mut self.fields {
            if let (InputId::Remote, Value::Text(current)) = (field.id, &field.value) {
                let current = current.text().to_owned();
                field.value = choice(remotes.iter().cloned(), &current);
            }
        }
        self
    }

    #[must_use]
    pub fn with_style(mut self, style: DialogStyle) -> Self {
        self.style = style;
        self
    }

    #[must_use]
    pub fn focus(&self) -> Focus {
        self.focus
    }

    /// The field index and message of the current validation error.
    #[must_use]
    pub fn error(&self) -> Option<(usize, &str)> {
        self.error.as_ref().map(|(i, m)| (*i, m.as_str()))
    }

    /// Shows `message` under the field `id` (e.g. a rejection from
    /// `git check-ref-format`) and focuses it.
    pub fn set_error(&mut self, id: InputId, message: &str) {
        if let Some(index) = self.fields.iter().position(|f| f.id == id) {
            self.error = Some((index, message.to_owned()));
            self.focus = Focus::Field(index);
        }
    }

    /// The current values, in field order, as `set_input` takes them.
    #[must_use]
    pub fn values(&self) -> Vec<(InputId, InputValue)> {
        self.fields
            .iter()
            .map(|f| (f.id, f.input_value()))
            .collect()
    }

    /// Tab/Shift-Tab (and Down/Up) move focus; the focused field handles
    /// editing keys; Enter submits (or presses the focused button); Esc cancels.
    pub fn handle_key(&mut self, key: Key) -> InputOutcome {
        match (key, self.focus) {
            (Key::Esc, _) | (Key::Enter, Focus::Button(Button::Cancel)) => {
                return InputOutcome::Cancelled;
            }
            (Key::Tab | Key::Down, _) => self.move_focus(1),
            (Key::BackTab | Key::Up, _) => self.move_focus(-1),
            (Key::Enter, _) => return self.submit(),
            (Key::Left | Key::Right, Focus::Button(button)) => {
                self.focus = Focus::Button(match button {
                    Button::Confirm => Button::Cancel,
                    Button::Cancel => Button::Confirm,
                });
            }
            (key, Focus::Field(index)) => self.edit(index, key),
            (_, Focus::Button(_)) => {}
        }
        InputOutcome::Pending
    }

    /// Clicks focus a field (flipping a toggle, cycling a choice) or press a
    /// button. Clicks elsewhere are ignored.
    pub fn handle_click(&mut self, hits: &InputHitMap, x: u16, y: u16) -> InputOutcome {
        match hits.hit(x, y) {
            InputHit::Field(index) => {
                self.focus = Focus::Field(index);
                if let Some(Field {
                    value: Value::Toggle(_) | Value::Choice { .. },
                    ..
                }) = self.fields.get(index)
                {
                    self.edit(index, Key::Char(' '));
                }
                InputOutcome::Pending
            }
            InputHit::Button(Button::Confirm) => self.submit(),
            InputHit::Button(Button::Cancel) => InputOutcome::Cancelled,
            InputHit::Frame | InputHit::Outside => InputOutcome::Pending,
        }
    }

    fn move_focus(&mut self, step: isize) {
        let order: Vec<Focus> = (0..self.fields.len())
            .map(Focus::Field)
            .chain([
                Focus::Button(Button::Confirm),
                Focus::Button(Button::Cancel),
            ])
            .collect();
        let current = order.iter().position(|f| *f == self.focus).unwrap_or(0);
        let len = order.len().cast_signed();
        let next = (current.cast_signed() + step)
            .rem_euclid(len)
            .cast_unsigned();
        self.focus = order[next];
    }

    fn edit(&mut self, index: usize, key: Key) {
        let Some(field) = self.fields.get_mut(index) else {
            return;
        };
        let changed = match &mut field.value {
            Value::Text(text) => {
                match key {
                    Key::Char(ch) => text.insert(ch),
                    Key::Backspace => text.backspace(),
                    Key::Delete => text.delete(),
                    Key::Left => text.left(),
                    Key::Right => text.right(),
                    Key::Home => text.home(),
                    Key::End => text.end(),
                    _ => return,
                }
                matches!(key, Key::Char(_) | Key::Backspace | Key::Delete)
            }
            Value::Toggle(on) => {
                if !matches!(key, Key::Char(' ') | Key::Left | Key::Right) {
                    return;
                }
                *on = !*on;
                true
            }
            Value::Choice { options, index } => {
                let len = options.len().max(1);
                match key {
                    Key::Char(' ') | Key::Right => *index = (*index + 1) % len,
                    Key::Left => *index = (*index + len - 1) % len,
                    _ => return,
                }
                true
            }
        };
        if changed {
            self.error = None;
        }
    }

    fn submit(&mut self) -> InputOutcome {
        let mut action = self.action.clone();
        for (index, (id, value)) in self.values().into_iter().enumerate() {
            if let Err(error) = action.set_input(id, value) {
                self.error = Some((index, error.to_string()));
                self.focus = Focus::Field(index);
                return InputOutcome::Pending;
            }
        }
        self.error = None;
        InputOutcome::Submitted(action)
    }

    /// Centered on `screen`, tall enough for every field.
    #[must_use]
    pub fn area(&self, screen: Rect) -> Rect {
        let rows: u16 = self.fields.iter().map(Field::rows).sum();
        // Border, blank, fields, blank, buttons, border.
        let height = rows.saturating_add(5).min(screen.height);
        centered(screen, MAX_WIDTH.min(screen.width), height)
    }

    pub fn render(&self, screen: Rect, buf: &mut Buffer, theme: &Theme) -> InputHitMap {
        let area = self.area(screen);
        let mut hits = InputHitMap {
            area,
            ..InputHitMap::default()
        };
        if area.width < 6 || area.height < 3 {
            return hits;
        }
        draw_frame(buf, area, &self.title, Style::default().fg(theme.muted));
        let inner = text_width(area.width);
        let x = area.x + 2;
        let last_row = area.bottom().saturating_sub(3);
        let text = Style::default().fg(theme.text);
        let boxed = text.bg(theme.selected_bg);
        let mut y = area.y + 2;

        for (index, field) in self.fields.iter().enumerate() {
            if y + field.rows() - 1 > last_row {
                break;
            }
            let focused = self.focus == Focus::Field(index);
            let label_style = if focused {
                text.add_modifier(Modifier::BOLD)
            } else {
                text
            };
            let row = Rect::new(x, y, u16::try_from(inner).unwrap_or(u16::MAX), 1);
            match &field.value {
                Value::Toggle(on) => {
                    let mark = if *on { "[x]" } else { "[ ]" };
                    buf.set_string(x, y, mark, if focused { boxed } else { text });
                    buf.set_string(
                        x + 4,
                        y,
                        fit(field.label, inner.saturating_sub(4)),
                        label_style,
                    );
                    hits.fields.push((row, index));
                    y += 1;
                    continue;
                }
                Value::Text(_) | Value::Choice { .. } => {
                    buf.set_string(x, y, fit(field.label, inner), label_style);
                }
            }
            let value_row = Rect { y: y + 1, ..row };
            match &field.value {
                Value::Text(field) => {
                    let (visible, cursor_col) = window(field, inner);
                    buf.set_string(x, value_row.y, fit(&visible, inner), boxed);
                    let cursor = Position::new(x + cursor_col, value_row.y);
                    if focused {
                        if let Some(cell) = buf.cell_mut(cursor) {
                            cell.set_style(boxed.add_modifier(Modifier::REVERSED));
                        }
                        hits.cursor = Some(cursor);
                    }
                }
                Value::Choice { options, index } => {
                    let current = options.get(*index).map_or("", String::as_str);
                    let shown = format!("< {current} >");
                    buf.set_string(
                        x,
                        value_row.y,
                        fit(&shown, inner),
                        if focused { boxed } else { text },
                    );
                }
                Value::Toggle(_) => {}
            }
            hits.fields.push((value_row, index));
            if let Some((_, message)) = self.error.as_ref().filter(|(i, _)| *i == index) {
                buf.set_string(
                    x,
                    y + 2,
                    fit(message, inner),
                    Style::default().fg(self.style.warning),
                );
            }
            y += 3;
        }

        let buttons = [
            (Button::Confirm, self.title.as_str(), text),
            (Button::Cancel, "Cancel", text),
        ];
        let focus = match self.focus {
            Focus::Button(button) => Some(button),
            Focus::Field(_) => None,
        };
        hits.buttons = draw_buttons(buf, area, area.bottom() - 2, &buttons, focus, theme);
        hits
    }
}

/// A choice over `options` starting at `current` (added first if missing).
fn choice(options: impl Iterator<Item = String>, current: &str) -> Value {
    let mut options: Vec<String> = options.collect();
    let index = options
        .iter()
        .position(|o| o == current)
        .unwrap_or_else(|| {
            options.insert(0, current.to_owned());
            0
        });
    Value::Choice { options, index }
}

/// The part of `field` that fits in `columns` with the cursor visible,
/// and the cursor's column within it. Scrolls so the cursor sits at most on
/// the last column.
pub(crate) fn window(field: &TextField, columns: usize) -> (String, u16) {
    let chars: Vec<char> = field.text().chars().collect();
    let cursor = field.cursor();
    let room = columns.saturating_sub(1);
    let mut start = cursor;
    let mut used = 0;
    while start > 0 && used + width(chars[start - 1]) <= room {
        start -= 1;
        used += width(chars[start]);
    }
    let visible: String = chars[start..].iter().collect();
    (visible, u16::try_from(used).unwrap_or(u16::MAX))
}

#[cfg(test)]
mod tests;
