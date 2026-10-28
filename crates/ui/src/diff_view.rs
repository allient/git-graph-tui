use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Clear, Widget};
use tgg_git::show::{Diff, FileChange, LineKind};

use crate::details::file_label;
use crate::{Key, Theme, fit};

/// Diff colors that aren't in [`Theme`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffStyle {
    pub added: Color,
    pub deleted: Color,
    /// `@@ ... @@` lines.
    pub hunk: Color,
}

impl Default for DiffStyle {
    fn default() -> Self {
        Self {
            added: Color::Green,
            deleted: Color::Red,
            hunk: Color::Cyan,
        }
    }
}

/// What a key press means for the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffOutcome {
    Stay,
    /// Back to the graph (or the details panel).
    Close,
}

/// What is under a screen cell, as of the last render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffHit {
    /// The path and counts row.
    Header,
    /// A diff line (index into the diff's lines).
    Line(usize),
    Outside,
}

/// Where the header and each visible diff line were drawn.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiffHitMap {
    area: Rect,
    lines: Vec<(Rect, usize)>,
}

impl DiffHitMap {
    #[must_use]
    pub fn hit(&self, x: u16, y: u16) -> DiffHit {
        let point = Position { x, y };
        if let Some((_, line)) = self.lines.iter().find(|(r, _)| r.contains(point)) {
            DiffHit::Line(*line)
        } else if self.area.contains(point) {
            DiffHit::Header
        } else {
            DiffHit::Outside
        }
    }

    /// Rows available for diff lines in the last render (the scroll page).
    #[must_use]
    pub fn page(&self) -> u16 {
        self.area.height.saturating_sub(1)
    }
}

/// The built-in diff view (S10): one file's unified diff with a header,
/// a line-number gutter and add/delete coloring, scrolled vertically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffView {
    title: String,
    counts: String,
    diff: Diff,
    offset: usize,
    style: DiffStyle,
}

impl DiffView {
    #[must_use]
    pub fn new(file: &FileChange, diff: Diff) -> Self {
        let (_, counts) = file_label(file);
        let title = match &file.old_path {
            Some(old) => format!("{old} → {}", file.path),
            None => file.path.clone(),
        };
        Self {
            title,
            counts,
            diff,
            offset: 0,
            style: DiffStyle::default(),
        }
    }

    #[must_use]
    pub fn with_style(mut self, style: DiffStyle) -> Self {
        self.style = style;
        self
    }

    /// Index of the first visible line.
    #[must_use]
    pub fn offset(&self) -> usize {
        self.offset
    }

    #[must_use]
    pub fn diff(&self) -> &Diff {
        &self.diff
    }

    /// Arrows scroll a line, PageUp/PageDown a page, Home/End to the ends;
    /// Esc or `q` closes. `page` is the number of visible diff rows
    /// ([`DiffHitMap::page`] from the last render).
    pub fn handle_key(&mut self, key: Key, page: u16) -> DiffOutcome {
        let page_rows = isize::try_from(page.max(1)).unwrap_or(1);
        match key {
            Key::Esc | Key::Char('q') => return DiffOutcome::Close,
            Key::Down => self.scroll_by(1, page),
            Key::Up => self.scroll_by(-1, page),
            Key::PageDown => self.scroll_by(page_rows, page),
            Key::PageUp => self.scroll_by(-page_rows, page),
            Key::Home => self.offset = 0,
            Key::End => self.offset = self.max_offset(page),
            _ => {}
        }
        DiffOutcome::Stay
    }

    /// Scrolls by `delta` lines (mouse wheel), keeping the last line at or
    /// below the bottom of a `page`-row view.
    pub fn scroll_by(&mut self, delta: isize, page: u16) {
        let target = self.offset.saturating_add_signed(delta);
        self.offset = target.min(self.max_offset(page));
    }

    fn max_offset(&self, page: u16) -> usize {
        self.diff.lines.len().saturating_sub(usize::from(page))
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) -> DiffHitMap {
        let mut hits = DiffHitMap {
            area,
            lines: Vec::new(),
        };
        if area.width < 2 || area.height == 0 {
            return hits;
        }
        Clear.render(area, buf);
        let width = usize::from(area.width - 1);
        let header = format!("{} {}", self.title, self.counts);
        let bold = Style::default().fg(theme.text).add_modifier(Modifier::BOLD);
        buf.set_string(area.x + 1, area.y, fit(&header, width), bold);

        let rows = usize::from(area.height - 1);
        if self.diff.is_binary() {
            let note = "Binary file, no text diff to show.";
            if rows > 0 {
                buf.set_string(
                    area.x + 1,
                    area.y + 1,
                    fit(note, width),
                    Style::default().fg(theme.muted),
                );
            }
            return hits;
        }
        let offset = self.offset.min(self.max_offset(area.height - 1));
        for (row, (index, line)) in self
            .diff
            .lines
            .iter()
            .enumerate()
            .skip(offset)
            .take(rows)
            .enumerate()
        {
            let y = area.y + 1 + u16::try_from(row).unwrap_or(u16::MAX);
            let number =
                |n: Option<u32>| n.map_or_else(|| "    ".to_owned(), |n| format!("{n:>4}"));
            let gutter = format!(" {} {} │ ", number(line.old_line), number(line.new_line));
            let style = Style::default().fg(match line.kind {
                LineKind::Added => self.style.added,
                LineKind::Deleted => self.style.deleted,
                LineKind::Hunk => self.style.hunk,
                LineKind::Header | LineKind::NoNewline => theme.muted,
                LineKind::Context => theme.text,
            });
            // Tabs would break cell alignment; long lines are cut, not wrapped.
            let text = line.text.replace('\t', "    ");
            buf.set_string(area.x, y, &gutter, Style::default().fg(theme.muted));
            let text_x = area.x + u16::try_from(gutter.chars().count()).unwrap_or(u16::MAX);
            let room = usize::from(area.right().saturating_sub(text_x));
            buf.set_string(text_x, y, fit(&text, room), style);
            hits.lines
                .push((Rect::new(area.x, y, area.width, 1), index));
        }
        hits
    }
}

#[cfg(test)]
mod tests;
