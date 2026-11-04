use ggt_actions::{Action, Choice, InputId, InputKind, InputValue};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::confirm::{MAX_WIDTH, centered, draw_buttons, draw_frame, text_width, wrap};
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
    /// An option of the open dropdown list.
    Option(usize),
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
    /// The open dropdown list and its option rows; drawn over the dialog.
    dropdown: Option<Rect>,
    options: Vec<(Rect, usize)>,
}

impl InputHitMap {
    #[must_use]
    pub fn hit(&self, x: u16, y: u16) -> InputHit {
        let point = Position { x, y };
        if let Some((_, i)) = self.options.iter().find(|(r, _)| r.contains(point)) {
            InputHit::Option(*i)
        } else if self.dropdown.is_some_and(|r| r.contains(point)) {
            InputHit::Frame
        } else if let Some((_, i)) = self.fields.iter().find(|(r, _)| r.contains(point)) {
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
    Choice {
        options: Vec<ChoiceOption>,
        index: usize,
    },
}

/// One option of a choice field. Remotes have no description.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ChoiceOption {
    value: String,
    description: &'static str,
    destructive: bool,
}

impl From<&Choice> for ChoiceOption {
    fn from(choice: &Choice) -> Self {
        Self {
            value: choice.value.to_owned(),
            description: choice.description,
            destructive: choice.destructive,
        }
    }
}

/// The open list of a choice field, drawn under it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Dropdown {
    field: usize,
    highlighted: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Field {
    id: InputId,
    label: &'static str,
    /// The dialog's question already says what this field is for.
    hide_label: bool,
    value: Value,
}

impl Field {
    /// Label, value and error rows for text and choices; one row for toggles.
    fn rows(&self) -> u16 {
        match self.value {
            Value::Toggle(_) => 1,
            Value::Text(_) | Value::Choice { .. } => 2 + self.label_rows(),
        }
    }

    fn label_rows(&self) -> u16 {
        u16::from(!self.hide_label)
    }

    fn input_value(&self) -> InputValue {
        match &self.value {
            Value::Text(field) => InputValue::Text(field.text().to_owned()),
            Value::Toggle(on) => InputValue::Bool(*on),
            Value::Choice { options, index } => InputValue::Text(
                options
                    .get(*index)
                    .map(|o| o.value.clone())
                    .unwrap_or_default(),
            ),
        }
    }
}

/// The dialog that asks for an action's inputs (branch name, tag message,
/// reset mode, ...), validates them and returns the completed action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputDialog {
    title: String,
    /// Asked above the fields, like VS Code's dialogs.
    question: Option<String>,
    confirm: String,
    action: Action,
    fields: Vec<Field>,
    focus: Focus,
    error: Option<(usize, String)>,
    style: DialogStyle,
    dropdown: Option<Dropdown>,
}

impl InputDialog {
    /// The dialog for `action`'s inputs, prefilled with its current values,
    /// or `None` when it has none. `title` is also the confirm button label
    /// (e.g. the menu label without `...`).
    #[must_use]
    pub fn for_action(title: &str, action: &Action) -> Option<Self> {
        let mut fields: Vec<Field> = action
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
                        choice(options.iter().map(ChoiceOption::from).collect(), &text)
                    }
                    _ => Value::Text(TextField::new(&text)),
                };
                Field {
                    id: spec.id,
                    label: spec.label,
                    hide_label: false,
                    value,
                }
            })
            .collect();
        let question = action.question();
        // A lone text field needs no label under a question that names it.
        if let [field] = fields.as_mut_slice()
            && question.is_some()
            && matches!(field.value, Value::Text(_))
        {
            field.hide_label = true;
        }
        (!fields.is_empty()).then(|| Self {
            title: title.to_owned(),
            question,
            confirm: action.confirm_label().unwrap_or(title).to_owned(),
            action: action.clone(),
            fields,
            focus: Focus::Field(0),
            error: None,
            style: DialogStyle::default(),
            dropdown: None,
        })
    }

    /// Offers the repository's remotes as a choice instead of free text.
    #[must_use]
    pub fn with_remotes(mut self, remotes: &[String]) -> Self {
        for field in &mut self.fields {
            if let (InputId::Remote, Value::Text(current)) = (field.id, &field.value) {
                let current = current.text().to_owned();
                let options = remotes.iter().map(|r| plain_option(r)).collect();
                field.value = choice(options, &current);
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
        if self.dropdown.is_some() {
            self.dropdown_key(key);
            return InputOutcome::Pending;
        }
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
        // An open list takes the click: an option picks it, anywhere else
        // only closes the list.
        if self.dropdown.is_some() {
            if let InputHit::Option(option) = hits.hit(x, y) {
                self.pick(option);
            }
            self.dropdown = None;
            return InputOutcome::Pending;
        }
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
            InputHit::Option(_) | InputHit::Frame | InputHit::Outside => InputOutcome::Pending,
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

    fn edit(&mut self, field_index: usize, key: Key) {
        let Some(field) = self.fields.get_mut(field_index) else {
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
            // Only Space: arrows on the way to a button must not tick, say,
            // "Force delete".
            Value::Toggle(on) => {
                if key != Key::Char(' ') {
                    return;
                }
                *on = !*on;
                true
            }
            Value::Choice { options, index } => {
                let len = options.len().max(1);
                match key {
                    // Space opens the list; the arrows still step through
                    // the options without it.
                    Key::Char(' ') => {
                        self.dropdown = Some(Dropdown {
                            field: field_index,
                            highlighted: *index,
                        });
                        return;
                    }
                    Key::Right => *index = (*index + 1) % len,
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

    /// Whether a choice field's list is open.
    #[must_use]
    pub fn is_dropdown_open(&self) -> bool {
        self.dropdown.is_some()
    }

    /// Up and Down move through the open list, Enter or Space pick, Esc or
    /// Tab close it without changing the value.
    fn dropdown_key(&mut self, key: Key) {
        let Some(Dropdown { field, highlighted }) = self.dropdown else {
            return;
        };
        let len = self.options(field).len();
        match key {
            Key::Up => self.highlight(field, highlighted.saturating_sub(1)),
            Key::Down => self.highlight(field, (highlighted + 1).min(len.saturating_sub(1))),
            Key::Home => self.highlight(field, 0),
            Key::End => self.highlight(field, len.saturating_sub(1)),
            Key::Enter | Key::Char(' ') => {
                self.pick(highlighted);
                self.dropdown = None;
            }
            Key::Esc | Key::Tab | Key::BackTab => self.dropdown = None,
            _ => {}
        }
    }

    fn highlight(&mut self, field: usize, highlighted: usize) {
        self.dropdown = Some(Dropdown { field, highlighted });
    }

    /// Sets the open list's field to `option`.
    fn pick(&mut self, option: usize) {
        let Some(Dropdown { field, .. }) = self.dropdown else {
            return;
        };
        if let Some(Field {
            value: Value::Choice { options, index },
            ..
        }) = self.fields.get_mut(field)
            && option < options.len()
        {
            *index = option;
            self.error = None;
        }
    }

    fn options(&self, field: usize) -> &[ChoiceOption] {
        match self.fields.get(field) {
            Some(Field {
                value: Value::Choice { options, .. },
                ..
            }) => options,
            _ => &[],
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
        let width = MAX_WIDTH.min(screen.width);
        let rows: u16 = self.fields.iter().map(Field::rows).sum();
        // Border, blank, question and a blank, fields, blank, buttons, border.
        let question = self.question_lines(width).len();
        let question = u16::try_from(question).map_or(u16::MAX, |q| q + u16::from(q > 0));
        let height = rows
            .saturating_add(question)
            .saturating_add(5)
            .min(screen.height);
        centered(screen, width, height)
    }

    fn question_lines(&self, width: u16) -> Vec<String> {
        self.question
            .as_deref()
            .map(|q| wrap(q, text_width(width)))
            .unwrap_or_default()
    }

    /// Draws the question centered from row `y`; returns the first row after
    /// it and its blank line.
    fn draw_question(&self, buf: &mut Buffer, area: Rect, mut y: u16, style: Style) -> u16 {
        let inner = text_width(area.width);
        let question = self.question_lines(area.width);
        for line in &question {
            if y > area.bottom().saturating_sub(3) {
                break;
            }
            let pad = inner.saturating_sub(line.width()) / 2;
            buf.set_string(
                area.x + 2 + u16::try_from(pad).unwrap_or(0),
                y,
                fit(line, inner - pad),
                style,
            );
            y += 1;
        }
        if question.is_empty() { y } else { y + 1 }
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

        y = self.draw_question(buf, area, y, text);

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
                Value::Text(_) | Value::Choice { .. } if !field.hide_label => {
                    buf.set_string(x, y, fit(field.label, inner), label_style);
                }
                Value::Text(_) | Value::Choice { .. } => {}
            }
            let value_row = Rect {
                y: y + field.label_rows(),
                ..row
            };
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
                Value::Choice {
                    options,
                    index: chosen,
                } => {
                    let open = self.dropdown.is_some_and(|d| d.field == index);
                    let style = if focused || open {
                        boxed
                    } else {
                        text.bg(theme.header_bg)
                    };
                    self.draw_choice(buf, value_row, options, *chosen, open, style, theme);
                }
                Value::Toggle(_) => {}
            }
            hits.fields.push((value_row, index));
            if let Some((_, message)) = self.error.as_ref().filter(|(i, _)| *i == index) {
                buf.set_string(
                    x,
                    value_row.y + 1,
                    fit(message, inner),
                    Style::default().fg(self.style.warning),
                );
            }
            y += field.rows();
        }

        let buttons = [
            (Button::Confirm, self.confirm.as_str(), text),
            (Button::Cancel, "Cancel", text),
        ];
        let focus = match self.focus {
            Focus::Button(button) => Some(button),
            Focus::Field(_) => None,
        };
        hits.buttons = draw_buttons(buf, area, area.bottom() - 2, &buttons, focus, theme);
        self.draw_dropdown(screen, buf, theme, &mut hits);
        hits
    }

    /// A dropdown button as wide as the longest option, with the chosen
    /// option's description beside it.
    #[expect(clippy::too_many_arguments, reason = "one row of render's state")]
    fn draw_choice(
        &self,
        buf: &mut Buffer,
        row: Rect,
        options: &[ChoiceOption],
        chosen: usize,
        open: bool,
        style: Style,
        theme: &Theme,
    ) {
        let inner = usize::from(row.width);
        let button = button_width(options).min(inner);
        let current = options.get(chosen);
        let value = current.map_or("", |o| o.value.as_str());
        let arrow = if open { "▴" } else { "▾" };
        let label = format!(" {} {arrow} ", fit(value, button.saturating_sub(4)));
        buf.set_string(row.x, row.y, fit(&label, button), style);
        if let Some(option) = current.filter(|o| !o.description.is_empty()) {
            let color = if option.destructive {
                self.style.warning
            } else {
                theme.muted
            };
            let rest = inner.saturating_sub(button + 2);
            buf.set_string(
                row.x + to_u16(button + 2),
                row.y,
                fit(option.description, rest),
                Style::default().fg(color),
            );
        }
    }

    /// The open list, hanging from its field's button like a menu: framed
    /// in the accent color, as wide as its options, the current option
    /// ticked, the highlighted one on a full-width bar, descriptions muted
    /// on the right (destructive ones in the warning color).
    fn draw_dropdown(&self, screen: Rect, buf: &mut Buffer, theme: &Theme, hits: &mut InputHitMap) {
        let Some(Dropdown { field, highlighted }) = self.dropdown else {
            return;
        };
        let Some(value_row) = hits.field(field) else {
            return;
        };
        let current = match self.fields.get(field) {
            Some(Field {
                value: Value::Choice { index, .. },
                ..
            }) => *index,
            _ => return,
        };
        let options = self.options(field);
        let values = options.iter().map(|o| o.value.width()).max().unwrap_or(0);
        let descriptions = options
            .iter()
            .map(|o| o.description.width())
            .max()
            .unwrap_or(0);
        // Border, space, tick, value, gap, description, space, border.
        let gap = if descriptions > 0 { 3 } else { 0 };
        let content = 1 + 2 + values + gap + descriptions + 1;
        let width = (content + 2).max(button_width(options) + 2);
        let height = to_u16(options.len()).saturating_add(2);
        let top = dropdown_top(value_row, height, screen);
        // One column left of the button, so the options line up with its text.
        let left = value_row.x.saturating_sub(1);
        let area = Rect::new(
            left,
            top,
            to_u16(width).min(screen.right().saturating_sub(left)),
            height,
        )
        .intersection(screen);
        if area.width < 6 || area.height < 3 {
            return;
        }
        draw_frame(buf, area, "", Style::default().fg(theme.lane(0)));
        let inner = usize::from(area.width).saturating_sub(2);
        for (i, option) in options.iter().enumerate() {
            let y = area.y + 1 + to_u16(i);
            if y >= area.bottom().saturating_sub(1) {
                break;
            }
            let row = Rect::new(area.x + 1, y, to_u16(inner), 1);
            let base = if i == highlighted {
                Style::default().fg(theme.text).bg(theme.selected_bg)
            } else {
                Style::default().fg(theme.text)
            };
            let tick = if i == current { "✓ " } else { "  " };
            let label = format!(" {tick}{}", option.value);
            buf.set_string(row.x, y, fit(&label, inner), base);
            if !option.description.is_empty() {
                let color = if option.destructive {
                    self.style.warning
                } else {
                    theme.muted
                };
                let at = inner.saturating_sub(1 + option.description.width());
                if at > label.width() + 1 {
                    buf.set_string(row.x + to_u16(at), y, option.description, base.fg(color));
                }
            }
            hits.options.push((row, i));
        }
        hits.dropdown = Some(area);
    }
}

/// The dropdown button: a space, the longest option, a space, `▾`, a space.
fn button_width(options: &[ChoiceOption]) -> usize {
    options.iter().map(|o| o.value.width()).max().unwrap_or(0) + 4
}

/// Where a list `height` rows tall starts: under `field` when it fits, else
/// above it when that fits, else under it, cut off by the screen. Never
/// over the field itself.
fn dropdown_top(field: Rect, height: u16, screen: Rect) -> u16 {
    let below = field.bottom();
    if below.saturating_add(height) <= screen.bottom() {
        below
    } else if field.y >= screen.y.saturating_add(height) {
        field.y - height
    } else {
        below
    }
}

fn to_u16(n: usize) -> u16 {
    u16::try_from(n).unwrap_or(u16::MAX)
}

/// A choice over `options` starting at `current` (added first if missing).
fn choice(mut options: Vec<ChoiceOption>, current: &str) -> Value {
    let index = options
        .iter()
        .position(|o| o.value == current)
        .unwrap_or_else(|| {
            options.insert(0, plain_option(current));
            0
        });
    Value::Choice { options, index }
}

/// An option without a description, such as a remote's name.
fn plain_option(value: &str) -> ChoiceOption {
    ChoiceOption {
        value: value.to_owned(),
        description: "",
        destructive: false,
    }
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
