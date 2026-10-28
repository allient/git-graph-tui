//! Full-screen and modal panels drawn over the graph: the info dialog and
//! the git output view.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Widget, Wrap};
use tgg_ui::Theme;

use crate::state::{GitOutput, Info, OutputLine};

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

/// The git output view over `area`; returns the rows it shows per page.
pub fn draw_output(
    frame: &mut Frame<'_>,
    area: Rect,
    output: &GitOutput,
    offset: usize,
    theme: &Theme,
) -> usize {
    Clear.render(area, frame.buffer_mut());
    let block = Block::bordered().title(" git output · esc closes ");
    let inner = block.inner(area);
    block.render(area, frame.buffer_mut());
    let page = usize::from(inner.height);
    let lines = output.lines();
    let offset = offset.min(lines.len().saturating_sub(page.max(1)));
    let text: Vec<Line<'_>> = lines
        .into_iter()
        .skip(offset)
        .take(page)
        .map(|line| match line {
            OutputLine::Command { command, status } => {
                let ok = status == "ok";
                let mark = if ok {
                    "✓".to_owned()
                } else {
                    format!("✗ {status}")
                };
                Line::from(vec![
                    Span::styled(
                        format!("$ {command}"),
                        Style::new().add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        mark,
                        Style::new().fg(if ok { theme.lane(2) } else { theme.lane(5) }),
                    ),
                ])
            }
            OutputLine::Stdout(text) => Line::from(format!("  {text}")),
            OutputLine::Stderr(text) => {
                Line::styled(format!("  {text}"), Style::new().fg(theme.muted))
            }
            OutputLine::Note(text) => Line::styled(text, Style::new().fg(theme.muted)),
        })
        .collect();
    Paragraph::new(text).render(inner, frame.buffer_mut());
    page
}
