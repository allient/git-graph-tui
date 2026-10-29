use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use unicode_width::UnicodeWidthStr;

use crate::{Theme, fit};

/// The banner's buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BannerButton {
    Continue,
    Abort,
}

const BUTTONS: [(BannerButton, &str); 2] = [
    (BannerButton::Continue, "[Continue]"),
    (BannerButton::Abort, "[Abort]"),
];

/// Columns between the message, the buttons and the key hints.
const GAP: &str = "   ";
const HINTS: &str = "C continue  A abort";

/// Where each button was drawn.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BannerHitMap {
    buttons: Vec<(Rect, BannerButton)>,
}

impl BannerHitMap {
    #[must_use]
    pub fn hit(&self, x: u16, y: u16) -> Option<BannerButton> {
        let point = Position { x, y };
        self.buttons
            .iter()
            .find(|(r, _)| r.contains(point))
            .map(|(_, b)| *b)
    }

    #[must_use]
    pub fn button(&self, button: BannerButton) -> Option<Rect> {
        self.buttons
            .iter()
            .find(|(_, b)| *b == button)
            .map(|(r, _)| *r)
    }
}

/// The one-line bar shown while git is in the middle of a merge, rebase,
/// cherry-pick or revert: what is going on, then `[Continue]` `[Abort]`
/// and their keys. On narrow terminals the hints go first, then the
/// message is cut; the buttons always show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Banner {
    message: String,
}

impl Banner {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Draws the banner on the first row of `area`.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) -> BannerHitMap {
        let mut hits = BannerHitMap::default();
        if area.width == 0 || area.height == 0 {
            return hits;
        }
        let y = area.y;
        let width = usize::from(area.width);
        let buttons_width =
            GAP.width() + BUTTONS.iter().map(|(_, l)| l.width()).sum::<usize>() + BUTTONS.len() - 1;
        let message_width = self.message.width();
        let room = width.saturating_sub(1 + buttons_width);
        let message = if message_width <= room {
            self.message.clone()
        } else {
            fit(&self.message, room)
        };
        let show_hints = 1 + message.width() + buttons_width + GAP.width() + HINTS.width() <= width;

        buf.set_style(
            Rect::new(area.x, y, area.width, 1),
            Style::default().bg(theme.header_bg),
        );
        let bold = Style::default()
            .fg(theme.lane(5))
            .add_modifier(Modifier::BOLD);
        let mut x = area.x + 1;
        buf.set_stringn(x, y, &message, width - 1, bold);
        x += to_u16(message.width() + GAP.width());
        for (button, label) in BUTTONS {
            let w = to_u16(label.width());
            if x + w > area.right() {
                break;
            }
            buf.set_string(x, y, label, Style::default().fg(theme.text));
            hits.buttons.push((Rect::new(x, y, w, 1), button));
            x += w + 1;
        }
        if show_hints {
            let x = x - 1 + to_u16(GAP.width());
            buf.set_string(x, y, HINTS, Style::default().fg(theme.muted));
        }
        hits
    }
}

fn to_u16(n: usize) -> u16 {
    u16::try_from(n).unwrap_or(u16::MAX)
}

#[cfg(test)]
mod tests;
