//! The framed list every menu-like widget draws: the context menus and the
//! dropdown lists of [`crate::Select`]. One look for both: a frame in the
//! accent color, the highlighted row on a full-width bar, disabled rows
//! muted, separators across the frame, and a detail (a submenu's `▸`, an
//! option's description) right-aligned.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Clear, Widget};
use unicode_width::UnicodeWidthStr;

use crate::{Theme, fit};

/// Space between the frame and the text, on each side.
pub(crate) const PADDING: usize = 1;

/// One row of a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ListRow<'a> {
    Item(ListItem<'a>),
    Separator,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ListItem<'a> {
    pub label: &'a str,
    /// Right-aligned: a submenu's `▸`, an option's description.
    pub detail: &'a str,
    /// The keyboard or mouse highlight.
    pub highlighted: bool,
    /// Shown but not selectable.
    pub disabled: bool,
    /// Loses work: the detail is drawn in the warning color.
    pub warning: bool,
}

/// Frame plus padding: what a list adds to its widest row.
pub(crate) const CHROME: usize = 2 + 2 * PADDING;

/// The width a row needs inside the frame and padding: its label, and its
/// detail after `gap` columns.
pub(crate) fn row_width(label: &str, detail: &str, gap: usize) -> usize {
    if detail.is_empty() {
        label.width()
    } else {
        label.width() + gap + detail.width()
    }
}

/// Draws `rows` from `offset` on in `area` (frame included), over whatever
/// is there. Returns where each item landed, with its index in `rows`.
pub(crate) fn draw_list(
    buf: &mut Buffer,
    area: Rect,
    rows: &[ListRow<'_>],
    offset: usize,
    theme: &Theme,
    warning: Color,
) -> Vec<(Rect, usize)> {
    let mut hits = Vec::new();
    if area.width < 2 || area.height < 2 {
        return hits;
    }
    Clear.render(area, buf);
    let frame = Style::default().fg(theme.menu_border);
    let inner = usize::from(area.width - 2);
    let (left, right) = (area.x, area.right() - 1);
    buf.set_string(left, area.y, border('┌', '─', '┐', inner), frame);
    buf.set_string(left, area.bottom() - 1, border('└', '─', '┘', inner), frame);
    let visible = usize::from(area.height - 2);
    for (row, (index, item)) in rows
        .iter()
        .enumerate()
        .skip(offset)
        .take(visible)
        .enumerate()
    {
        let y = area.y + 1 + u16::try_from(row).unwrap_or(u16::MAX);
        let ListRow::Item(item) = item else {
            buf.set_string(left, y, border('├', '─', '┤', inner), frame);
            continue;
        };
        let style = if item.disabled {
            Style::default().fg(theme.muted)
        } else if item.highlighted {
            Style::default().fg(theme.text).bg(theme.selected_bg)
        } else {
            Style::default().fg(theme.text)
        };
        let room = inner.saturating_sub(2 * PADDING);
        let detail_room = if item.detail.is_empty() {
            0
        } else {
            item.detail.width() + 1
        };
        let pad = " ".repeat(PADDING);
        let label = fit(item.label, room.saturating_sub(detail_room));
        buf.set_string(left, y, "│", frame);
        buf.set_string(left + 1, y, format!("{pad}{label}"), style);
        if detail_room > 0 {
            let detail = if item.disabled {
                style
            } else if item.warning {
                style.fg(warning)
            } else if item.detail == "▸" {
                style
            } else {
                style.fg(theme.muted)
            };
            let at = inner.saturating_sub(PADDING + item.detail.width());
            let gap = at.saturating_sub(PADDING + label.width());
            buf.set_string(
                left + 1 + to_u16(PADDING + label.width()),
                y,
                " ".repeat(gap),
                style,
            );
            buf.set_string(left + 1 + to_u16(at), y, item.detail, detail);
            buf.set_string(left + 1 + to_u16(inner - PADDING), y, pad.as_str(), style);
        } else {
            buf.set_string(
                left + 1 + to_u16(PADDING + label.width()),
                y,
                pad.as_str(),
                style,
            );
        }
        buf.set_string(right, y, "│", frame);
        hits.push((Rect::new(left + 1, y, area.width - 2, 1), index));
    }
    hits
}

fn border(left: char, fill: char, right: char, inner: usize) -> String {
    let mut line = String::with_capacity(inner + 2);
    line.push(left);
    line.extend(std::iter::repeat_n(fill, inner));
    line.push(right);
    line
}

fn to_u16(n: usize) -> u16 {
    u16::try_from(n).unwrap_or(u16::MAX)
}
