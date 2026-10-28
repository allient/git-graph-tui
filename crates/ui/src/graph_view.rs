use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use tgg_graph::Row;
use tgg_model::{Commit, Oid, Ref};

use crate::badges::{BadgeKind, badges_for};
use crate::glyphs::lane_glyphs;
use crate::text::{fit, format_date};
use crate::theme::Theme;

const HASH_WIDTH: u16 = 8;
const DATE_WIDTH: u16 = 16;
const AUTHOR_WIDTH: u16 = 14;

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

/// Screen regions and what they show, recorded while rendering.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HitMap {
    regions: Vec<(Rect, HitTarget)>,
    gap: Option<Rect>,
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

    /// Where the [`Gap`] was opened, if its row was on screen.
    #[must_use]
    pub fn gap(&self) -> Option<Rect> {
        self.gap
    }

    fn push(&mut self, area: Rect, target: HitTarget) {
        self.regions.push((area, target));
    }
}

/// The commit graph: lanes, ref badges, subject, date, author and hash.
///
/// Display rows are the optional "Uncommitted Changes" row followed by the
/// commits. `offset` and `selected` are display-row indices.
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

    /// Draws the visible rows into `area` and returns what each region shows.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) -> HitMap {
        let mut hits = HitMap::default();
        let columns = Columns::for_width(area, self.lane_width(area));

        let mut y = area.top();
        let mut display = self.offset;
        while y < area.bottom() && display < self.len() {
            let row_area = Rect::new(area.x, y, area.width, 1);
            if display == self.selected {
                buf.set_style(row_area, Style::new().bg(theme.selected_bg));
            }
            match self.commit_index(display) {
                None => {
                    self.draw_uncommitted(buf, &columns, y, theme);
                    hits.push(row_area, HitTarget::Uncommitted);
                }
                Some(index) => {
                    hits.push(row_area, HitTarget::Commit(index));
                    self.draw_commit(buf, &columns, y, index, theme, &mut hits);
                }
            }
            y += 1;
            if let Some(gap) = self.gap.filter(|g| g.after == display) {
                let height = gap.height.min(area.bottom() - y);
                hits.gap = Some(Rect::new(area.x, y, area.width, height));
                y += height;
            }
            display += 1;
        }
        hits
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
        let x = cols.graph.x + u16::try_from(2 * self.head_lane()).unwrap_or(u16::MAX);
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
            for (x, cell) in (cols.graph.x..cols.graph.right()).zip(lane_glyphs(row, cols.lanes)) {
                let target = &mut buf[(x, y)];
                target.set_char(cell.ch);
                if let Some(color) = cell.color {
                    target.set_fg(theme.lane(color));
                }
            }
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
    fn for_width(area: Rect, lanes: usize) -> Self {
        let lanes = lanes.max(1);
        let graph_width = u16::try_from(2 * lanes).unwrap_or(u16::MAX).min(area.width);
        let col = |show: bool, width: u16| show.then_some(width);
        let hash = col(area.width >= 60, HASH_WIDTH);
        let date = col(area.width >= 80, DATE_WIDTH);
        let author = col(area.width >= 100, AUTHOR_WIDTH);
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
