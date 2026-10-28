//! Full-screen and modal panels drawn over the graph: the info dialog and
//! the git output view.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, Paragraph, Widget, Wrap};
use tgg_ui::Theme;

use crate::state::Info;

/// Widest the info dialog gets.
const INFO_WIDTH: u16 = 72;

/// The info dialog, centered on `screen`.
pub fn draw_info(frame: &mut Frame<'_>, screen: Rect, info: &Info, theme: &Theme) {
    let width = INFO_WIDTH.min(screen.width.saturating_sub(2)).max(1);
    // Wrapped lines can need more rows than lines; leave room for a few.
    let wanted = u16::try_from(info.lines.len())
        .unwrap_or(u16::MAX)
        .saturating_add(4);
    let height = wanted.min(screen.height.saturating_sub(2)).max(1);
    let area = Rect::new(
        screen.x + (screen.width - width) / 2,
        screen.y + (screen.height - height) / 2,
        width,
        height,
    );
    Clear.render(area, frame.buffer_mut());
    let block = Block::bordered()
        .border_style(Style::new().fg(theme.muted))
        .title(format!(" {} · esc closes ", info.title));
    let text: Vec<Line<'_>> = info.lines.iter().map(|l| Line::from(l.as_str())).collect();
    Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: false })
        .render(area, frame.buffer_mut());
}
