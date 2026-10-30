use std::cell::Cell;

use ggt_git::show::{Diff, DiffLine, FileChange, LineKind};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Clear, Widget};

use crate::details::file_label;
use crate::{Key, Theme, fit};

/// Narrowest diff area that shows old and new side by side.
pub const SIDE_BY_SIDE_WIDTH: u16 = 100;

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
    /// A diff line (index into the diff's lines; the old side of a
    /// side-by-side row when it has one).
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

    /// Columns the last render had, to tell whether side by side fits.
    #[must_use]
    pub fn width(&self) -> u16 {
        self.area.width
    }
}

/// One screen row of the view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Row {
    /// The header of a file in a multi-file view (index into `files`).
    File(usize),
    /// A line across the whole width: every line of the unified view; hunk
    /// and other header lines side by side.
    Line(usize),
    /// Side by side: the old line on the left, the new one on the right;
    /// None leaves that side blank.
    Pair(Option<usize>, Option<usize>),
}

/// The built-in diff view (S10): a unified diff with a header, a
/// line-number gutter and add/delete coloring, scrolled vertically. It can
/// show old and new side by side, and the diffs of several files one after
/// the other, each under its own header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffView {
    title: String,
    counts: String,
    diff: Diff,
    /// Where each file of a multi-file view starts in `diff.lines`, and the
    /// label of its header row. Empty for a single file.
    files: Vec<(usize, String)>,
    /// First visible row.
    offset: usize,
    style: DiffStyle,
    side_by_side: bool,
    /// A plain text file (a file at a revision), never shown side by side.
    plain: bool,
    /// The last frame was wide enough for side by side.
    wide: Cell<bool>,
}

impl DiffView {
    #[must_use]
    pub fn new(file: &FileChange, diff: Diff) -> Self {
        let (_, counts) = file_label(file);
        let title = match &file.old_path {
            Some(old) => format!("{old} → {}", file.path),
            None => file.path.clone(),
        };
        Self::build(title, counts, diff)
    }

    /// One file's diff under `title`, with the counts taken from the diff
    /// itself (uncommitted changes have no numstat).
    #[must_use]
    pub fn titled(title: String, diff: Diff) -> Self {
        let counts = counts(&diff);
        Self::build(title, counts, diff)
    }

    /// The diffs of several files in one view, each under a header row
    /// with its label (a path and a status letter), all scrolled together.
    #[must_use]
    pub fn multi(title: String, files: Vec<(String, Diff)>) -> Self {
        let mut lines = Vec::new();
        let mut starts = Vec::new();
        for (label, diff) in files {
            starts.push((lines.len(), label));
            lines.extend(diff.lines);
        }
        let diff = Diff { lines };
        let mut view = Self::titled(title, diff);
        view.files = starts;
        view
    }

    /// A plain text file (a file at a revision): every line is a context
    /// line numbered from 1 in the new-line column, with no `+`/`-` counts.
    #[must_use]
    pub fn text(title: String, content: &str) -> Self {
        let lines = content
            .lines()
            .zip(1u32..)
            .map(|(text, n)| DiffLine {
                kind: LineKind::Context,
                text: text.to_owned(),
                old_line: None,
                new_line: Some(n),
            })
            .collect();
        let mut view = Self::build(title, String::new(), Diff { lines });
        view.plain = true;
        view
    }

    fn build(title: String, counts: String, diff: Diff) -> Self {
        Self {
            title,
            counts,
            diff,
            files: Vec::new(),
            offset: 0,
            style: DiffStyle::default(),
            side_by_side: false,
            plain: false,
            wide: Cell::new(true),
        }
    }

    #[must_use]
    pub fn with_style(mut self, style: DiffStyle) -> Self {
        self.style = style;
        self
    }

    /// Shows old and new side by side (when the area is at least
    /// [`SIDE_BY_SIDE_WIDTH`] wide) or as one unified column.
    #[must_use]
    pub fn with_side_by_side(mut self, on: bool) -> Self {
        self.set_side_by_side(on);
        self
    }

    pub fn set_side_by_side(&mut self, on: bool) {
        self.side_by_side = on && !self.plain;
    }

    #[must_use]
    pub fn side_by_side(&self) -> bool {
        self.side_by_side
    }

    /// False for a plain text file, which has no sides.
    #[must_use]
    pub fn can_split(&self) -> bool {
        !self.plain
    }

    /// Index of the first visible row.
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

    /// Scrolls by `delta` rows (mouse wheel), keeping the last row at or
    /// below the bottom of a `page`-row view.
    pub fn scroll_by(&mut self, delta: isize, page: u16) {
        let target = self.offset.saturating_add_signed(delta);
        self.offset = target.min(self.max_offset(page));
    }

    fn max_offset(&self, page: u16) -> usize {
        self.rows().len().saturating_sub(usize::from(page))
    }

    /// Whether the rows are laid out side by side (asked for, and it fit).
    fn split(&self) -> bool {
        self.side_by_side && self.wide.get()
    }

    /// The rows to draw: file headers, then each file's lines; side by side,
    /// deleted and added runs are paired line by line and the `diff --git`,
    /// `index`, `---` and `+++` lines are left out.
    fn rows(&self) -> Vec<Row> {
        let split = self.split();
        let lines = &self.diff.lines;
        let mut files = self
            .files
            .iter()
            .map(|(start, _)| *start)
            .enumerate()
            .peekable();
        let mut rows = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            while let Some((file, _)) = files.next_if(|(_, start)| *start <= i) {
                rows.push(Row::File(file));
            }
            let line = &lines[i];
            if !split {
                rows.push(Row::Line(i));
                i += 1;
                continue;
            }
            match line.kind {
                LineKind::Context => {
                    rows.push(Row::Pair(Some(i), Some(i)));
                    i += 1;
                }
                LineKind::Deleted | LineKind::Added => {
                    let (mut old, mut new) = (Vec::new(), Vec::new());
                    while let Some(line) = lines.get(i) {
                        match line.kind {
                            LineKind::Deleted => old.push(i),
                            LineKind::Added => new.push(i),
                            _ => break,
                        }
                        i += 1;
                    }
                    for k in 0..old.len().max(new.len()) {
                        rows.push(Row::Pair(old.get(k).copied(), new.get(k).copied()));
                    }
                }
                LineKind::Header if is_patch_header(&line.text) => i += 1,
                _ => {
                    rows.push(Row::Line(i));
                    i += 1;
                }
            }
        }
        // Files whose diff has no lines still get their header.
        rows.extend(files.map(|(file, _)| Row::File(file)));
        rows
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) -> DiffHitMap {
        let mut hits = DiffHitMap {
            area,
            lines: Vec::new(),
        };
        if area.width < 2 || area.height == 0 {
            return hits;
        }
        self.wide.set(area.width >= SIDE_BY_SIDE_WIDTH);
        Clear.render(area, buf);
        let width = usize::from(area.width - 1);
        let header = if self.counts.is_empty() {
            self.title.clone()
        } else {
            format!("{} {}", self.title, self.counts)
        };
        let bold = Style::default().fg(theme.text).add_modifier(Modifier::BOLD);
        buf.set_string(area.x + 1, area.y, fit(&header, width), bold);

        let rows = usize::from(area.height - 1);
        if self.files.is_empty() && self.diff.is_binary() {
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
        let all = self.rows();
        let offset = self
            .offset
            .min(all.len().saturating_sub(usize::from(area.height - 1)));
        let split = self.split();
        for (row, item) in all.iter().skip(offset).take(rows).enumerate() {
            let y = area.y + 1 + u16::try_from(row).unwrap_or(u16::MAX);
            let line_area = Rect::new(area.x, y, area.width, 1);
            match *item {
                Row::File(file) => {
                    let style = Style::default()
                        .fg(theme.text)
                        .bg(theme.header_bg)
                        .add_modifier(Modifier::BOLD);
                    buf.set_style(line_area, style);
                    let label = format!("▾ {}", self.files[file].1);
                    buf.set_string(area.x + 1, y, fit(&label, width), style);
                }
                Row::Line(index) if split => {
                    let line = &self.diff.lines[index];
                    let text = line.text.replace('\t', "    ");
                    buf.set_string(
                        area.x + 1,
                        y,
                        fit(&text, width),
                        self.line_style(line, theme),
                    );
                    hits.lines.push((line_area, index));
                }
                Row::Line(index) => {
                    self.draw_unified(buf, line_area, &self.diff.lines[index], theme);
                    hits.lines.push((line_area, index));
                }
                Row::Pair(old, new) => {
                    self.draw_pair(buf, line_area, old, new, theme);
                    if let Some(index) = old.or(new) {
                        hits.lines.push((line_area, index));
                    }
                }
            }
        }
        hits
    }

    fn line_style(&self, line: &DiffLine, theme: &Theme) -> Style {
        Style::default().fg(match line.kind {
            LineKind::Added => self.style.added,
            LineKind::Deleted => self.style.deleted,
            LineKind::Hunk => self.style.hunk,
            LineKind::Header | LineKind::NoNewline => theme.muted,
            LineKind::Context => theme.text,
        })
    }

    /// One line with both line numbers in the gutter.
    fn draw_unified(&self, buf: &mut Buffer, area: Rect, line: &DiffLine, theme: &Theme) {
        let gutter = format!(" {} {} │ ", number(line.old_line), number(line.new_line));
        // Tabs would break cell alignment; long lines are cut, not wrapped.
        let text = line.text.replace('\t', "    ");
        buf.set_string(area.x, area.y, &gutter, Style::default().fg(theme.muted));
        let text_x = area.x + u16::try_from(gutter.chars().count()).unwrap_or(u16::MAX);
        let room = usize::from(area.right().saturating_sub(text_x));
        buf.set_string(
            text_x,
            area.y,
            fit(&text, room),
            self.line_style(line, theme),
        );
    }

    /// The old line in the left half and the new one in the right half,
    /// each with its own line number, split by a `│`.
    fn draw_pair(
        &self,
        buf: &mut Buffer,
        area: Rect,
        old: Option<usize>,
        new: Option<usize>,
        theme: &Theme,
    ) {
        let left = area.width / 2;
        let muted = Style::default().fg(theme.muted);
        buf.set_string(area.x + left, area.y, "│", muted);
        let sides = [
            (old, Rect::new(area.x, area.y, left, 1), true),
            (
                new,
                Rect::new(area.x + left + 1, area.y, area.width - left - 1, 1),
                false,
            ),
        ];
        for (index, side, is_old) in sides {
            let Some(line) = index.map(|i| &self.diff.lines[i]) else {
                continue;
            };
            let n = if is_old { line.old_line } else { line.new_line };
            let gutter = format!("{} ", number(n));
            buf.set_string(side.x, side.y, &gutter, muted);
            // Without the `+`/`-`/` ` prefix: the side says which it is.
            let text: String = line.text.chars().skip(1).collect();
            let text = text.replace('\t', "    ");
            let text_x = side.x + u16::try_from(gutter.chars().count()).unwrap_or(u16::MAX);
            let room = usize::from(side.right().saturating_sub(text_x));
            buf.set_string(
                text_x,
                side.y,
                fit(&text, room),
                self.line_style(line, theme),
            );
        }
    }
}

/// A line number right-aligned in four columns, or blanks.
fn number(n: Option<u32>) -> String {
    n.map_or_else(|| "    ".to_owned(), |n| format!("{n:>4}"))
}

/// `(+added -deleted)`, or `(binary)`.
fn counts(diff: &Diff) -> String {
    if diff.is_binary() && diff.added() == 0 && diff.deleted() == 0 {
        "(binary)".to_owned()
    } else {
        format!("(+{} -{})", diff.added(), diff.deleted())
    }
}

/// The lines of a patch header that side by side has no use for.
fn is_patch_header(text: &str) -> bool {
    ["diff --git ", "index ", "--- ", "+++ "]
        .iter()
        .any(|prefix| text.starts_with(prefix))
}

#[cfg(test)]
mod tests;
