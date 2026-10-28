use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Clear, Widget};
use tgg_actions::{Action, Confirmation};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{Key, Theme, fit};

/// Widest a dialog gets, in columns (narrower on small terminals).
pub const MAX_WIDTH: u16 = 60;

/// Dialog colors that aren't in [`Theme`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialogStyle {
    /// Border, title and confirm button of destructive dialogs; validation errors.
    pub warning: Color,
}

impl Default for DialogStyle {
    fn default() -> Self {
        Self {
            warning: Color::LightRed,
        }
    }
}

/// A dialog button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Confirm,
    Cancel,
}

/// What a key press or click means for the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogOutcome {
    /// Still open.
    Pending,
    Confirmed,
    Cancelled,
}

/// What is under a screen cell, as of the last render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogHit {
    Button(Button),
    /// Inside the dialog but not on a button.
    Frame,
    Outside,
}

/// Where the dialog and its buttons were drawn.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DialogHitMap {
    area: Rect,
    buttons: Vec<(Rect, Button)>,
}

impl DialogHitMap {
    #[must_use]
    pub fn hit(&self, x: u16, y: u16) -> DialogHit {
        let point = Position { x, y };
        if let Some((_, button)) = self.buttons.iter().find(|(r, _)| r.contains(point)) {
            DialogHit::Button(*button)
        } else if self.area.contains(point) {
            DialogHit::Frame
        } else {
            DialogHit::Outside
        }
    }

    #[must_use]
    pub fn area(&self) -> Rect {
        self.area
    }

    /// Where `button` was drawn, if it fit.
    #[must_use]
    pub fn button(&self, button: Button) -> Option<Rect> {
        self.buttons
            .iter()
            .find(|(_, b)| *b == button)
            .map(|(r, _)| *r)
    }
}

/// The S13 confirmation shown before an action runs, centered on screen.
///
/// Destructive actions are drawn as a warning and focus Cancel, so pressing
/// Enter by reflex never deletes or rewrites anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmDialog {
    confirmation: Confirmation,
    destructive: bool,
    focus: Button,
    style: DialogStyle,
}

impl ConfirmDialog {
    /// The dialog for `action`, or `None` when it runs without confirmation.
    #[must_use]
    pub fn for_action(action: &Action) -> Option<Self> {
        Some(Self::new(action.confirmation()?, action.is_destructive()))
    }

    #[must_use]
    pub fn new(confirmation: Confirmation, destructive: bool) -> Self {
        Self {
            confirmation,
            destructive,
            focus: if destructive {
                Button::Cancel
            } else {
                Button::Confirm
            },
            style: DialogStyle::default(),
        }
    }

    #[must_use]
    pub fn with_style(mut self, style: DialogStyle) -> Self {
        self.style = style;
        self
    }

    #[must_use]
    pub fn confirmation(&self) -> &Confirmation {
        &self.confirmation
    }

    #[must_use]
    pub fn is_destructive(&self) -> bool {
        self.destructive
    }

    #[must_use]
    pub fn focus(&self) -> Button {
        self.focus
    }

    /// Left/Right/Tab/Shift-Tab move focus, Enter presses it, Esc cancels.
    pub fn handle_key(&mut self, key: Key) -> DialogOutcome {
        match key {
            Key::Left | Key::Right | Key::Tab | Key::BackTab => {
                self.focus = match self.focus {
                    Button::Confirm => Button::Cancel,
                    Button::Cancel => Button::Confirm,
                };
                DialogOutcome::Pending
            }
            Key::Enter => match self.focus {
                Button::Confirm => DialogOutcome::Confirmed,
                Button::Cancel => DialogOutcome::Cancelled,
            },
            Key::Esc => DialogOutcome::Cancelled,
            _ => DialogOutcome::Pending,
        }
    }

    /// A click on a button answers; a click anywhere else is ignored, so a
    /// stray click can't dismiss (or confirm) a destructive action.
    pub fn handle_click(&mut self, hits: &DialogHitMap, x: u16, y: u16) -> DialogOutcome {
        match hits.hit(x, y) {
            DialogHit::Button(Button::Confirm) => DialogOutcome::Confirmed,
            DialogHit::Button(Button::Cancel) => DialogOutcome::Cancelled,
            DialogHit::Frame | DialogHit::Outside => DialogOutcome::Pending,
        }
    }

    /// Centered on `screen`, as tall as the wrapped message needs.
    #[must_use]
    pub fn area(&self, screen: Rect) -> Rect {
        let width = MAX_WIDTH.min(screen.width);
        let lines = wrap(&self.confirmation.message, text_width(width)).len();
        // Border, blank, message, blank, buttons, border.
        let height = u16::try_from(lines + 5)
            .unwrap_or(u16::MAX)
            .min(screen.height);
        centered(screen, width, height)
    }

    pub fn render(&self, screen: Rect, buf: &mut Buffer, theme: &Theme) -> DialogHitMap {
        let area = self.area(screen);
        let mut hits = DialogHitMap {
            area,
            buttons: Vec::new(),
        };
        if area.width < 4 || area.height < 3 {
            return hits;
        }
        let accent = if self.destructive {
            Style::default().fg(self.style.warning)
        } else {
            Style::default().fg(theme.muted)
        };
        draw_frame(buf, area, &self.confirmation.title, accent);

        let text = Style::default().fg(theme.text);
        let body_rows = usize::from(area.height.saturating_sub(5));
        let lines = wrap(&self.confirmation.message, text_width(area.width));
        for (row, line) in lines.iter().take(body_rows).enumerate() {
            let y = area.y + 2 + u16::try_from(row).unwrap_or(u16::MAX);
            buf.set_string(area.x + 2, y, line, text);
        }

        let y = area.bottom() - 2;
        let confirm_style = if self.destructive { accent } else { text };
        let buttons = [
            (
                Button::Confirm,
                self.confirmation.confirm_label.as_str(),
                confirm_style,
            ),
            (Button::Cancel, "Cancel", text),
        ];
        hits.buttons = draw_buttons(buf, area, y, &buttons, Some(self.focus), theme);
        hits
    }
}

/// Columns available for text inside a dialog `width` wide (border + padding).
pub(crate) fn text_width(width: u16) -> usize {
    usize::from(width.saturating_sub(4))
}

pub(crate) fn centered(screen: Rect, width: u16, height: u16) -> Rect {
    Rect::new(
        screen.x + (screen.width - width) / 2,
        screen.y + (screen.height - height) / 2,
        width,
        height,
    )
}

/// Clears `area` and draws a border with `title` in the top edge.
pub(crate) fn draw_frame(buf: &mut Buffer, area: Rect, title: &str, style: Style) {
    Clear.render(area, buf);
    let inner = usize::from(area.width - 2);
    let title = fit(&format!(" {title} "), inner.min(title.width() + 2));
    let top = format!(
        "┌{title}{}┐",
        "─".repeat(inner.saturating_sub(title.width()))
    );
    buf.set_string(area.x, area.y, top, style);
    for y in area.y + 1..area.bottom() - 1 {
        buf.set_string(area.x, y, "│", style);
        buf.set_string(area.right() - 1, y, "│", style);
    }
    buf.set_string(
        area.x,
        area.bottom() - 1,
        format!("└{}┘", "─".repeat(inner)),
        style,
    );
}

/// Draws `[ label ]` buttons right-aligned on row `y`, highlighting `focus`,
/// (if any), and returns where each one landed.
pub(crate) fn draw_buttons<B: Copy + PartialEq>(
    buf: &mut Buffer,
    area: Rect,
    y: u16,
    buttons: &[(B, &str, Style)],
    focus: Option<B>,
    theme: &Theme,
) -> Vec<(Rect, B)> {
    let labels: Vec<String> = buttons
        .iter()
        .map(|(_, label, _)| format!("[ {label} ]"))
        .collect();
    // One space between buttons and one before the right border.
    let total: usize = labels
        .iter()
        .map(|l| l.width() + 1)
        .sum::<usize>()
        .saturating_sub(1);
    let mut x = area
        .right()
        .saturating_sub(2)
        .saturating_sub(u16::try_from(total).unwrap_or(u16::MAX))
        .max(area.x + 1);
    let mut rects = Vec::new();
    for ((id, _, style), label) in buttons.iter().zip(labels) {
        let width = u16::try_from(label.width()).unwrap_or(u16::MAX);
        if x + width > area.right() - 1 {
            break;
        }
        let style = if Some(*id) == focus {
            style.bg(theme.selected_bg)
        } else {
            *style
        };
        buf.set_string(x, y, &label, style);
        rects.push((Rect::new(x, y, width, 1), *id));
        x += width + 1;
    }
    rects
}

/// Word-wraps `text` to `width` columns, splitting words longer than a line.
pub(crate) fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    if width == 0 {
        return lines;
    }
    let mut line = String::new();
    for word in text.split_whitespace() {
        for piece in split_to_width(word, width) {
            let used = line.width();
            if used == 0 {
                line = piece;
            } else if used + 1 + piece.width() <= width {
                line.push(' ');
                line.push_str(&piece);
            } else {
                lines.push(std::mem::replace(&mut line, piece));
            }
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

fn split_to_width(word: &str, width: usize) -> Vec<String> {
    let mut pieces = vec![String::new()];
    let mut used = 0;
    for ch in word.chars() {
        let w = ch.width().unwrap_or(0);
        if used + w > width && used > 0 {
            pieces.push(String::new());
            used = 0;
        }
        if let Some(last) = pieces.last_mut() {
            last.push(ch);
        }
        used += w;
    }
    pieces
}

#[cfg(test)]
mod tests;
