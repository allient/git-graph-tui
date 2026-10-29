use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use tgg_actions::GraphColumn;
use tgg_graph::Row;
use tgg_model::{Commit, Oid, Ref};

use crate::badges::{Badge, BadgeKind, badges_for};
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
            ColumnDivider::BeforeColumn(column) => {
                *w.width_mut(column) = shift(self.width(column), -delta, MAX_COLUMN);
            }
            ColumnDivider::Between(left, right) => {
                let total = self.width(left) + self.width(right);
                let moved = shift(self.width(left), delta, total - MIN_COLUMN);
                *w.width_mut(left) = moved;
                *w.width_mut(right) = total - moved;
            }
        }
        w
    }

    fn width(self, column: GraphColumn) -> u16 {
        match column {
            GraphColumn::Date => self.date,
            GraphColumn::Author => self.author,
            GraphColumn::Commit => self.hash,
        }
    }

    fn width_mut(&mut self, column: GraphColumn) -> &mut u16 {
        match column {
            GraphColumn::Date => &mut self.date,
            GraphColumn::Author => &mut self.author,
            GraphColumn::Commit => &mut self.hash,
        }
    }
}

/// A divider in the column header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnDivider {
    /// Between the graph and the description: resizes the graph.
    AfterGraph,
    /// Between the description and the first shown column: resizes it.
    BeforeColumn(GraphColumn),
    /// Between two shown columns: trades width between them.
    Between(GraphColumn, GraphColumn),
}

/// Screen regions and what they show, recorded while rendering.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HitMap {
    regions: Vec<(Rect, HitTarget)>,
    gap: Option<Rect>,
    dividers: Vec<(u16, u16, ColumnDivider)>,
    header: Option<Rect>,
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

    /// Whether a cell is on the column header line (right-click shows or
    /// hides columns).
    #[must_use]
    pub fn is_header(&self, x: u16, y: u16) -> bool {
        self.header
            .is_some_and(|area| area.contains(ratatui::layout::Position { x, y }))
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
    /// Commits matching the find bar, ascending; their subjects are highlighted.
    pub matches: &'a [usize],
    /// Columns hidden from the header's menu.
    pub hidden: &'a [GraphColumn],
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
        let columns = Columns::for_width(area, self.lane_width(area), self.widths, self.hidden);
        hits.graph_width = columns.graph.width;
        hits.header = Some(Rect::new(area.x, area.y, area.width, 1));
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
                // The lanes carry on through the gap, like in VS Code: the
                // panel starts at the Description column.
                if let Some(row) = self.commit_index(display).and_then(|i| self.rows.get(i)) {
                    let cells = gap_glyphs(row, columns.lanes);
                    for line in y..y + height {
                        draw_glyphs(buf, &columns, line, &cells, theme);
                    }
                }
                let x = columns.description.x;
                hits.gap = Some(Rect::new(x, y, area.right().saturating_sub(x), height));
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
            let start = x;
            let remotes;
            (x, remotes) = draw_badge(buf, x, y, right, &badge, lane_color, theme);
            if x > start {
                hits.push(
                    Rect::new(start, y, x - start, 1),
                    HitTarget::Ref {
                        commit: index,
                        full_name: badge.full_name,
                    },
                );
            }
            // Pushed after the badge, so a remote's name wins over it.
            for (from, to, remote) in remotes.into_iter().filter(|(from, to, _)| to > from) {
                hits.push(
                    Rect::new(from, y, to - from, 1),
                    HitTarget::Ref {
                        commit: index,
                        full_name: format!("refs/remotes/{remote}/{}", badge.label),
                    },
                );
            }
            x = put(buf, x, y, right, " ", Style::new());
        }
        let subject_style = if self.matches.binary_search(&index).is_ok() {
            Style::new().fg(theme.tag).add_modifier(Modifier::BOLD)
        } else {
            text_style
        };
        put(buf, x, y, right, &commit.subject, subject_style);

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

/// Draws one ref badge from `x` and returns where it ends, with the
/// columns (start, end) each remote's name covers. The current
/// branch gets a circle before it and a bold name; remote names are italic.
/// With Nerd icons the badge is VS Code's: rounded ends, an icon on the lane
/// color, then the name on `badge_bg`.
fn draw_badge(
    buf: &mut Buffer,
    mut x: u16,
    y: u16,
    right: u16,
    badge: &Badge,
    lane_color: usize,
    theme: &Theme,
) -> (u16, Vec<(u16, u16, String)>) {
    let color = match badge.kind {
        BadgeKind::Tag => theme.tag,
        BadgeKind::Stash => theme.stash,
        _ => theme.lane(lane_color),
    };
    let head = badge.kind == BadgeKind::Head;
    if head {
        x = put(buf, x, y, right, "○ ", Style::new().fg(color));
    }
    let icon = theme.icons.ref_badge(badge.kind);
    let ends = theme.icons.badge_ends();
    // Without an icon the whole badge is on the lane color.
    let body_bg = if icon.is_some() {
        theme.badge_bg
    } else {
        color
    };
    let body = |style: Style| style.fg(theme.badge_text(body_bg)).bg(body_bg);
    let name = if head {
        body(Style::new()).add_modifier(Modifier::BOLD)
    } else {
        body(Style::new())
    };
    let remote = body(Style::new()).add_modifier(Modifier::ITALIC);

    if let Some((left, _)) = ends {
        x = put(buf, x, y, right, &left.to_string(), Style::new().fg(color));
    }
    if let Some(icon) = icon {
        // A blank cell after the icon lets a wide Nerd glyph overflow.
        let style = Style::new().fg(theme.badge_text(color)).bg(color);
        x = put(buf, x, y, right, &format!("{icon} "), style);
    }
    x = put(buf, x, y, right, &format!(" {} ", badge.label), name);
    let mut remotes = Vec::new();
    for name in &badge.remotes {
        x = put(buf, x, y, right, "│", body(Style::new()));
        let start = x;
        x = put(buf, x, y, right, &format!(" {name} "), remote);
        remotes.push((start, x, name.clone()));
    }
    if let Some((_, end)) = ends {
        x = put(buf, x, y, right, &end.to_string(), Style::new().fg(body_bg));
    }
    (x, remotes)
}

/// The lines leaving a commit, run straight down through the gap opened
/// below it: each in the lane it leaves the commit line in (a fork already
/// turned to its new lane there), in its color.
fn gap_glyphs(row: &Row, width: usize) -> Vec<GlyphCell> {
    let width = width.max(row.width()).max(1);
    let blank = GlyphCell {
        ch: ' ',
        color: None,
    };
    let mut cells = vec![blank; 2 * width - 1];
    let mut edges: Vec<_> = row.bottom.iter().collect();
    // Straight lanes first, so a fork joining a lane doesn't recolor it.
    edges.sort_by_key(|e| e.from != e.to);
    for e in edges {
        let lane = if e.from == row.lane { e.to } else { e.from };
        if let Some(cell) = cells.get_mut(2 * lane).filter(|c| c.color.is_none()) {
            *cell = GlyphCell {
                ch: '│',
                color: Some(e.color),
            };
        }
    }
    cells
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
    // Hidden columns have no divider: each shown one gets one on its left.
    let mut left = None;
    for (column, area) in cols.optional() {
        if area.is_some() {
            let which = left.map_or(ColumnDivider::BeforeColumn(column), |left| {
                ColumnDivider::Between(left, column)
            });
            divider(area, which);
            left = Some(column);
        }
    }
    dividers
}

/// Writes `text` from `x` up to (not including) `right`; returns the next x.
fn put(buf: &mut Buffer, x: u16, y: u16, right: u16, text: &str, style: Style) -> u16 {
    if x >= right {
        return x;
    }
    buf.set_stringn(x, y, text, usize::from(right - x), style).0
}

/// Horizontal layout of one line. Optional columns appear as the width allows
/// (hash from 60 columns, date from 80, author from 100) and unless hidden.
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
    /// The optional columns, left to right, and where each is (if shown).
    fn optional(&self) -> [(GraphColumn, Option<Rect>); 3] {
        [
            (GraphColumn::Date, self.date),
            (GraphColumn::Author, self.author),
            (GraphColumn::Commit, self.hash),
        ]
    }

    /// Where lane 0 is drawn, after the padding.
    fn lanes_x(&self) -> u16 {
        self.graph.x + GRAPH_PADDING.min(self.graph.width)
    }

    fn for_width(area: Rect, lanes: usize, widths: ColumnWidths, hidden: &[GraphColumn]) -> Self {
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
        let col = |column: GraphColumn, min_width: u16, width: u16| {
            (area.width >= min_width && !hidden.contains(&column)).then_some(width)
        };
        let hash = col(GraphColumn::Commit, 60, widths.hash);
        let date = col(GraphColumn::Date, 80, widths.date);
        let author = col(GraphColumn::Author, 100, widths.author);
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
        let w = start().dragged(ColumnDivider::BeforeColumn(GraphColumn::Date), -4);
        assert_eq!(w.date, DATE_WIDTH + 4, "moving left widens the date");
        assert_eq!((w.author, w.hash), (AUTHOR_WIDTH, HASH_WIDTH));
    }

    #[test]
    fn dividers_next_to_a_hidden_column_resize_the_shown_ones() {
        let w = start().dragged(ColumnDivider::BeforeColumn(GraphColumn::Author), -2);
        assert_eq!((w.date, w.author), (DATE_WIDTH, AUTHOR_WIDTH + 2));
        let w = start().dragged(
            ColumnDivider::Between(GraphColumn::Date, GraphColumn::Commit),
            -3,
        );
        assert_eq!(
            (w.date, w.author, w.hash),
            (DATE_WIDTH - 3, AUTHOR_WIDTH, HASH_WIDTH + 3)
        );
    }

    #[test]
    fn dragging_between_two_columns_trades_width_between_them() {
        let w = start().dragged(
            ColumnDivider::Between(GraphColumn::Date, GraphColumn::Author),
            3,
        );
        assert_eq!((w.date, w.author), (DATE_WIDTH + 3, AUTHOR_WIDTH - 3));
        let w = start().dragged(
            ColumnDivider::Between(GraphColumn::Author, GraphColumn::Commit),
            100,
        );
        assert_eq!(
            (w.author, w.hash),
            (AUTHOR_WIDTH + HASH_WIDTH - MIN_COLUMN, MIN_COLUMN)
        );
    }
}
