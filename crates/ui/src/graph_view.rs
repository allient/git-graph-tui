use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use tgg_graph::Row;
use tgg_model::{Commit, Oid, Ref};

use crate::badges::{BadgeKind, badges_for};
use crate::glyphs::{GlyphCell, connector_glyphs, lane_glyphs};
use crate::text::{fit, format_date};
use crate::theme::Theme;

const HASH_WIDTH: u16 = 8;
const DATE_WIDTH: u16 = 16;
const AUTHOR_WIDTH: u16 = 14;
/// Room for the "Graph" header and its divider.
const MIN_GRAPH_WIDTH: u16 = 7;
/// Blank columns left of the lanes.
const GRAPH_PADDING: u16 = 2;
/// Narrowest and widest a dragged column gets.
const MIN_COLUMN: u16 = 4;
const MAX_COLUMN: u16 = 80;

/// Screen lines per display row: its line and the connector below it.
pub const ROW_HEIGHT: u16 = 2;

/// What is under a screen cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HitTarget {
    /// The "Uncommitted Changes" row.
    Uncommitted,
    /// A commit row (index into the commits slice).
    Commit(usize),
    /// A ref badge on a commit row.
    Ref { commit: usize, full_name: String },
}

/// Column widths, changed by dragging the header's dividers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnWidths {
    /// None sizes the graph column to its lanes.
    pub graph: Option<u16>,
    pub date: u16,
    pub author: u16,
    pub hash: u16,
}

impl Default for ColumnWidths {
    fn default() -> Self {
        Self {
            graph: None,
            date: DATE_WIDTH,
            author: AUTHOR_WIDTH,
            hash: HASH_WIDTH,
        }
    }
}

impl ColumnWidths {
    /// The widths after moving `divider` by `delta` columns (negative is left),
    /// from these widths at the start of the drag. A divider between two fixed
    /// columns trades width between them; the description takes what is left.
    #[must_use]
    pub fn dragged(self, divider: ColumnDivider, delta: i32) -> Self {
        let shift = |width: u16, by: i32, max: u16| -> u16 {
            let moved = i32::from(width).saturating_add(by);
            u16::try_from(moved.clamp(i32::from(MIN_COLUMN), i32::from(max))).unwrap_or(MIN_COLUMN)
        };
        let mut w = self;
        match divider {
            ColumnDivider::AfterGraph => {
                w.graph = Some(shift(
                    self.graph.unwrap_or(MIN_GRAPH_WIDTH),
                    delta,
                    MAX_COLUMN,
                ));
            }
            ColumnDivider::AfterDescription => w.date = shift(self.date, -delta, MAX_COLUMN),
            ColumnDivider::AfterDate => {
                let total = self.date + self.author;
                w.date = shift(self.date, delta, total - MIN_COLUMN);
                w.author = total - w.date;
            }
            ColumnDivider::AfterAuthor => {
                let total = self.author + self.hash;
                w.author = shift(self.author, delta, total - MIN_COLUMN);
                w.hash = total - w.author;
            }
        }
        w
    }
}

/// A divider in the column header, named by the column on its left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnDivider {
    AfterGraph,
    AfterDescription,
    AfterDate,
    AfterAuthor,
}

/// Screen regions and what they show, recorded while rendering.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HitMap {
    regions: Vec<(Rect, HitTarget)>,
    gap: Option<Rect>,
    dividers: Vec<(u16, u16, ColumnDivider)>,
    graph_width: u16,
}

impl HitMap {
    /// The most specific target at a cell (a badge wins over its row).
    #[must_use]
    pub fn hit(&self, x: u16, y: u16) -> Option<&HitTarget> {
        let point = ratatui::layout::Position { x, y };
        self.regions
            .iter()
            .rev()
            .find(|(area, _)| area.contains(point))
            .map(|(_, target)| target)
    }

    /// The header divider drawn at a cell, for dragging.
    #[must_use]
    pub fn divider_at(&self, x: u16, y: u16) -> Option<ColumnDivider> {
        self.dividers
            .iter()
            .find(|&&(dx, dy, _)| (dx, dy) == (x, y))
            .map(|&(_, _, divider)| divider)
    }

    /// Width of the graph column as last drawn.
    #[must_use]
    pub fn graph_width(&self) -> u16 {
        self.graph_width
    }

    /// Where the [`Gap`] was opened, if its row was on screen.
    #[must_use]
    pub fn gap(&self) -> Option<Rect> {
        self.gap
    }

    /// Where the first region showing `target` was drawn.
    #[must_use]
    pub fn area_of(&self, target: &HitTarget) -> Option<Rect> {
        self.regions
            .iter()
            .find(|(_, t)| t == target)
            .map(|(area, _)| *area)
    }

    fn push(&mut self, area: Rect, target: HitTarget) {
        self.regions.push((area, target));
    }
}

/// The commit graph: a header naming the columns, then lanes, ref badges,
/// subject, date, author and hash.
///
/// Display rows are the optional "Uncommitted Changes" row followed by the
/// commits. Each takes [`ROW_HEIGHT`] lines: its own, then a connector line
/// carrying the lanes down to the next row. `offset` and `selected` are
/// display-row indices.
#[derive(Debug, Clone)]
pub struct GraphView<'a> {
    pub commits: &'a [Commit],
    /// Layout rows, one per commit (see `tgg_graph::Graph`).
    pub rows: &'a [Row],
    pub refs: &'a [Ref],
    /// The commit HEAD points at, for placing the uncommitted row.
    pub head: Option<&'a Oid>,
    /// Number of changed files; 0 hides the uncommitted row.
    pub uncommitted: usize,
    pub offset: usize,
    pub selected: usize,
    /// Blank lines opened below one row, for the inline details panel.
    pub gap: Option<Gap>,
    pub widths: ColumnWidths,
}

/// Blank lines opened below display row `after`; rows below it move down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gap {
    pub after: usize,
    pub height: u16,
}

impl GraphView<'_> {
    /// Number of display rows (uncommitted row plus commits).
    #[must_use]
    pub fn len(&self) -> usize {
        self.commits.len() + usize::from(self.uncommitted > 0)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Display rows that fit in `lines` screen lines, header included. The
    /// last row's connector line may be cut off.
    #[must_use]
    pub fn rows_fitting(lines: u16) -> usize {
        usize::from(lines.saturating_sub(1).div_ceil(ROW_HEIGHT)).max(1)
    }

    /// Draws the header and the visible rows into `area` and returns what each
    /// region shows.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) -> HitMap {
        let mut hits = HitMap::default();
        if area.is_empty() {
            return hits;
        }
        let columns = Columns::for_width(area, self.lane_width(area), self.widths);
        hits.graph_width = columns.graph.width;
        hits.dividers = draw_header(buf, &columns, area, theme);

        let mut y = area.top() + 1;
        let mut display = self.offset;
        while y < area.bottom() && display < self.len() {
            let row_area = Rect::new(area.x, y, area.width, 1);
            if display == self.selected {
                buf.set_style(row_area, Style::new().bg(theme.selected_bg));
            }
            // The row's region goes first so its badges win a hit.
            let target = match self.commit_index(display) {
                None => HitTarget::Uncommitted,
                Some(index) => HitTarget::Commit(index),
            };
            hits.push(row_area, target.clone());
            match self.commit_index(display) {
                None => self.draw_uncommitted(buf, &columns, y, theme),
                Some(index) => self.draw_commit(buf, &columns, y, index, theme, &mut hits),
            }
            y += 1;
            if let Some(gap) = self.gap.filter(|g| g.after == display) {
                let height = gap.height.min(area.bottom() - y);
                hits.gap = Some(Rect::new(area.x, y, area.width, height));
                y += height;
            }
            if y < area.bottom() && display + 1 < self.len() {
                self.draw_connector(buf, &columns, y, display, theme);
                hits.push(Rect::new(area.x, y, area.width, 1), target);
                y += 1;
            }
            display += 1;
        }
        hits
    }

    /// The line below display row `display`, joining it to the next row.
    fn draw_connector(
        &self,
        buf: &mut Buffer,
        cols: &Columns,
        y: u16,
        display: usize,
        theme: &Theme,
    ) {
        let Some(index) = self.commit_index(display) else {
            // A dotted line from the uncommitted row down to HEAD's lane.
            let x = cols.lanes_x() + u16::try_from(2 * self.head_lane()).unwrap_or(u16::MAX);
            if x < cols.graph.right() {
                buf[(x, y)].set_char('┆').set_fg(theme.muted);
            }
            return;
        };
        let Some(row) = self.rows.get(index) else {
            return;
        };
        let cells = connector_glyphs(row, self.rows.get(index + 1), cols.lanes);
        draw_glyphs(buf, cols, y, &cells, theme);
    }

    /// Commit index for a display row; None for the uncommitted row.
    fn commit_index(&self, display: usize) -> Option<usize> {
        if self.uncommitted > 0 {
            display.checked_sub(1)
        } else {
            Some(display)
        }
    }

    fn head_lane(&self) -> usize {
        self.head
            .and_then(|head| self.commits.iter().position(|c| &c.id == head))
            .and_then(|i| self.rows.get(i))
            .map_or(0, |r| r.lane)
    }

    /// Lanes needed by the visible rows.
    fn lane_width(&self, area: Rect) -> usize {
        let visible = self.offset..self.offset + usize::from(area.height);
        visible
            .filter_map(|d| self.commit_index(d))
            .filter_map(|i| self.rows.get(i))
            .map(Row::width)
            .chain([self.head_lane() + 1])
            .max()
            .unwrap_or(1)
    }

    fn draw_uncommitted(&self, buf: &mut Buffer, cols: &Columns, y: u16, theme: &Theme) {
        let x = cols.lanes_x() + u16::try_from(2 * self.head_lane()).unwrap_or(u16::MAX);
        if x < cols.graph.right() {
            buf[(x, y)].set_char('○').set_fg(theme.text);
        }
        let label = format!("Uncommitted Changes ({})", self.uncommitted);
        buf.set_stringn(
            cols.description.x,
            y,
            fit(&label, cols.description.width.into()),
            cols.description.width.into(),
            Style::new().fg(theme.text).add_modifier(Modifier::BOLD),
        );
    }

    fn draw_commit(
        &self,
        buf: &mut Buffer,
        cols: &Columns,
        y: u16,
        index: usize,
        theme: &Theme,
        hits: &mut HitMap,
    ) {
        let commit = &self.commits[index];
        let row = self.rows.get(index);
        let muted = row.is_some_and(|r| !r.in_head_ancestry);
        let text_style = Style::new().fg(if muted { theme.muted } else { theme.text });
        let lane_color = row.map_or(0, |r| r.color);

        if let Some(row) = row {
            draw_glyphs(buf, cols, y, &lane_glyphs(row, cols.lanes), theme);
        }

        // Badges, then the subject, clipped to the description column.
        let mut x = cols.description.x;
        let right = cols.description.right();
        for badge in badges_for(&commit.id, self.refs) {
            let bg = match badge.kind {
                BadgeKind::Tag => theme.tag,
                BadgeKind::Stash => theme.stash,
                _ => theme.lane(lane_color),
            };
            let mut style = Style::new().fg(theme.badge_fg).bg(bg);
            if badge.kind == BadgeKind::Head {
                style = style.add_modifier(Modifier::BOLD);
            }
            let text: String = std::iter::once(format!(" {} ", badge.label))
                .chain(badge.remotes.iter().map(|remote| format!("│ {remote} ")))
                .collect();
            let start = x;
            x = put(buf, x, y, right, &text, style);
            if x > start {
                hits.push(
                    Rect::new(start, y, x - start, 1),
                    HitTarget::Ref {
                        commit: index,
                        full_name: badge.full_name,
                    },
                );
            }
            x = put(buf, x, y, right, " ", Style::new());
        }
        put(buf, x, y, right, &commit.subject, text_style);

        let muted_or = |color| Style::new().fg(if muted { theme.muted } else { color });
        if let Some(area) = cols.date {
            let date = format_date(commit.author.time);
            put(buf, area.x, y, area.right(), &date, muted_or(theme.date));
        }
        if let Some(area) = cols.author {
            let name = fit(&commit.author.name, area.width.into());
            put(buf, area.x, y, area.right(), &name, muted_or(theme.author));
        }
        if let Some(area) = cols.hash {
            put(
                buf,
                area.x,
                y,
                area.right(),
                commit.id.short(7),
                muted_or(theme.hash),
            );
        }
    }
}

fn draw_glyphs(buf: &mut Buffer, cols: &Columns, y: u16, cells: &[GlyphCell], theme: &Theme) {
    for (x, cell) in (cols.lanes_x()..cols.graph.right()).zip(cells) {
        let target = &mut buf[(x, y)];
        target.set_char(cell.ch);
        if let Some(color) = cell.color {
            target.set_fg(theme.lane(color));
        }
    }
}

/// The column names and the dividers between them, on the first line of
/// `area`. Returns where each divider was drawn.
fn draw_header(
    buf: &mut Buffer,
    cols: &Columns,
    area: Rect,
    theme: &Theme,
) -> Vec<(u16, u16, ColumnDivider)> {
    let y = area.y;
    let style = Style::new()
        .fg(theme.text)
        .bg(theme.header_bg)
        .add_modifier(Modifier::BOLD);
    buf.set_style(Rect::new(area.x, y, area.width, 1), style);
    let mut label = |column: Option<Rect>, text: &str| {
        if let Some(c) = column {
            put(buf, c.x, y, c.right(), text, style);
        }
    };
    label(Some(cols.graph), "Graph");
    label(Some(cols.description), "Description");
    label(cols.date, "Date");
    label(cols.author, "Author");
    label(cols.hash, "Commit");

    // Each divider sits in the cell just left of the column after it.
    let mut dividers = Vec::new();
    let mut divider = |next: Option<Rect>, which: ColumnDivider| {
        if let Some(next) = next.filter(|n| n.x > area.x) {
            let x = next.x - 1;
            buf[(x, y)].set_char('│').set_fg(theme.muted);
            dividers.push((x, y, which));
        }
    };
    let description = Some(cols.description).filter(|d| d.width > 0);
    divider(description, ColumnDivider::AfterGraph);
    divider(cols.date, ColumnDivider::AfterDescription);
    divider(cols.date.and(cols.author), ColumnDivider::AfterDate);
    divider(cols.author.and(cols.hash), ColumnDivider::AfterAuthor);
    dividers
}

/// Writes `text` from `x` up to (not including) `right`; returns the next x.
fn put(buf: &mut Buffer, x: u16, y: u16, right: u16, text: &str, style: Style) -> u16 {
    if x >= right {
        return x;
    }
    buf.set_stringn(x, y, text, usize::from(right - x), style).0
}

/// Horizontal layout of one line. Optional columns appear as the width allows:
/// hash from 60 columns, date from 80, author from 100.
#[derive(Debug)]
struct Columns {
    lanes: usize,
    graph: Rect,
    description: Rect,
    date: Option<Rect>,
    author: Option<Rect>,
    hash: Option<Rect>,
}

impl Columns {
    /// Where lane 0 is drawn, after the padding.
    fn lanes_x(&self) -> u16 {
        self.graph.x + GRAPH_PADDING.min(self.graph.width)
    }

    fn for_width(area: Rect, lanes: usize, widths: ColumnWidths) -> Self {
        let lanes = lanes.max(1);
        let graph_width = widths
            .graph
            .unwrap_or_else(|| {
                u16::try_from(2 * lanes)
                    .unwrap_or(u16::MAX)
                    .saturating_add(GRAPH_PADDING)
                    .max(MIN_GRAPH_WIDTH)
            })
            .min(area.width);
        let col = |show: bool, width: u16| show.then_some(width);
        let hash = col(area.width >= 60, widths.hash);
        let date = col(area.width >= 80, widths.date);
        let author = col(area.width >= 100, widths.author);
        // Each optional column is preceded by one space.
        let fixed: u16 = [date, author, hash].iter().flatten().map(|w| w + 1).sum();
        let description_width = area.width.saturating_sub(graph_width + fixed);

        let mut x = area.x + graph_width + description_width;
        let mut place = |width: Option<u16>| {
            width.map(|w| {
                let rect = Rect::new(x + 1, area.y, w, 1);
                x += w + 1;
                rect
            })
        };
        let (date, author, hash) = (place(date), place(author), place(hash));
        Self {
            lanes,
            graph: Rect::new(area.x, area.y, graph_width, 1),
            description: Rect::new(area.x + graph_width, area.y, description_width, 1),
            date,
            author,
            hash,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn start() -> ColumnWidths {
        ColumnWidths {
            graph: Some(7),
            ..ColumnWidths::default()
        }
    }

    #[test]
    fn dragging_after_the_graph_resizes_the_graph_column() {
        let w = start().dragged(ColumnDivider::AfterGraph, 5);
        assert_eq!(w.graph, Some(12));
        let w = start().dragged(ColumnDivider::AfterGraph, -20);
        assert_eq!(w.graph, Some(MIN_COLUMN), "never narrower than the minimum");
    }

    #[test]
    fn dragging_after_the_description_resizes_the_date_column() {
        let w = start().dragged(ColumnDivider::AfterDescription, -4);
        assert_eq!(w.date, DATE_WIDTH + 4, "moving left widens the date");
        assert_eq!((w.author, w.hash), (AUTHOR_WIDTH, HASH_WIDTH));
    }

    #[test]
    fn dragging_between_two_columns_trades_width_between_them() {
        let w = start().dragged(ColumnDivider::AfterDate, 3);
        assert_eq!((w.date, w.author), (DATE_WIDTH + 3, AUTHOR_WIDTH - 3));
        let w = start().dragged(ColumnDivider::AfterAuthor, 100);
        assert_eq!(
            (w.author, w.hash),
            (AUTHOR_WIDTH + HASH_WIDTH - MIN_COLUMN, MIN_COLUMN)
        );
    }
}
