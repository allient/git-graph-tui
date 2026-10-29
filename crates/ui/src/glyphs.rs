use tgg_graph::Row;

/// One terminal cell of the graph column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphCell {
    pub ch: char,
    /// Lane color index (the theme maps it to a palette color); None for blanks.
    pub color: Option<usize>,
}

/// Directions a cell's lines go, plus the colors of its vertical and horizontal parts.
#[derive(Debug, Clone, Copy, Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "four independent line directions, matched together in box_char"
)]
struct Joints {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
    vertical: Option<usize>,
    horizontal: Option<usize>,
}

/// Draws a commit's line: the node and the lanes passing through it, `width`
/// lanes wide. Lanes turn on the connector lines between commits (see
/// [`connector_glyphs`]), so this line only has vertical lines.
///
/// Lane `i` is cell `2 * i`; the odd cells between lanes only carry
/// horizontal lines. The result has `2 * width - 1` cells.
#[must_use]
pub fn lane_glyphs(row: &Row, width: usize) -> Vec<GlyphCell> {
    let width = width.max(row.width()).max(1);
    let mut joints = vec![Joints::default(); 2 * width - 1];
    for edge in &row.top {
        let cell = &mut joints[2 * edge.to];
        cell.up = true;
        cell.vertical.get_or_insert(edge.color);
    }
    for edge in &row.bottom {
        let cell = &mut joints[2 * edge.from];
        cell.down = true;
        cell.vertical.get_or_insert(edge.color);
    }
    let mut cells = render(&joints);
    cells[2 * row.lane] = GlyphCell {
        ch: '●',
        color: Some(row.color),
    };
    cells
}

/// Draws the line between commit `above` and the commit `below` it (None when
/// `above` is the last row loaded), `width` lanes wide.
///
/// Each line leaving `above` runs down to where it enters `below`; a line that
/// changes lanes turns here with rounded corners, like `┠─╮` for a merge or
/// `┠─╯` for a branch joining its parent.
#[must_use]
pub fn connector_glyphs(above: &Row, below: Option<&Row>, width: usize) -> Vec<GlyphCell> {
    let width = width
        .max(above.width())
        .max(below.map_or(0, Row::width))
        .max(1);
    let mut joints = vec![Joints::default(); 2 * width - 1];
    let mut segments = segments(above, below);
    // Straight lanes first, so a curve crossing or leaving a lane doesn't
    // recolor it.
    segments.sort_by_key(|&(upper, lower, _)| upper != lower);
    for (upper, lower, color) in segments {
        let top = &mut joints[2 * upper];
        top.up = true;
        top.vertical.get_or_insert(color);
        let bottom = &mut joints[2 * lower];
        bottom.down = true;
        bottom.vertical.get_or_insert(color);
        span(&mut joints, upper, lower, color);
    }
    render(&joints)
}

/// The lines crossing the boundary between two rows, as (lane above, lane
/// below, color). The color is the one of the part that changes lanes.
fn segments(above: &Row, below: Option<&Row>) -> Vec<(usize, usize, usize)> {
    let entering = below.map_or(&[][..], |b| b.top.as_slice());
    let mut segments = Vec::new();
    for out in &above.bottom {
        let mut joined = entering.iter().filter(|e| e.from == out.to).peekable();
        if joined.peek().is_none() {
            segments.push((out.from, out.to, out.color));
        }
        for e in joined {
            let color = if out.from == out.to {
                e.color
            } else {
                out.color
            };
            segments.push((out.from, e.to, color));
        }
    }
    // Lines entering `below` that nothing above leads to.
    for e in entering {
        if !above.bottom.iter().any(|out| out.to == e.from) {
            segments.push((e.from, e.to, e.color));
        }
    }
    segments
}

/// Adds a horizontal line between lanes `a` and `b`.
fn span(joints: &mut [Joints], a: usize, b: usize, color: usize) {
    if a == b {
        return;
    }
    let (lo, hi) = (2 * a.min(b), 2 * a.max(b));
    joints[lo].right = true;
    joints[hi].left = true;
    for cell in &mut joints[lo..=hi] {
        cell.horizontal.get_or_insert(color);
    }
    for cell in &mut joints[lo + 1..hi] {
        cell.left = true;
        cell.right = true;
    }
}

fn render(joints: &[Joints]) -> Vec<GlyphCell> {
    joints
        .iter()
        .map(|j| {
            let ch = box_char(j);
            GlyphCell {
                ch,
                color: j.vertical.or(j.horizontal).filter(|_| ch != ' '),
            }
        })
        .collect()
}

/// Lanes are heavy vertical lines (`┃`, and `┠ ┨ ╂ ┰ ┸` where a line
/// branches off); horizontals stay light and turns use the rounded light
/// corners (`╭ ╮ ╰ ╯`), as Unicode has no heavy rounded ones.
fn box_char(j: &Joints) -> char {
    match (j.up, j.down, j.left, j.right) {
        (true, true, true, true) => '╂',
        (true, true, true, false) => '┨',
        (true, true, false, true) => '┠',
        (true, false, true, true) => '┸',
        (false, true, true, true) => '┰',
        (true, false, true, false) => '╯',
        (true, false, false, true) => '╰',
        (false, true, true, false) => '╮',
        (false, true, false, true) => '╭',
        (true, _, false, false) | (false, true, false, false) => '┃',
        (false, false, true, _) | (false, false, _, true) => '─',
        (false, false, false, false) => ' ',
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tgg_graph::{Edge, Row};

    fn e(from: usize, to: usize, color: usize) -> Edge {
        Edge { from, to, color }
    }

    fn row(lane: usize, color: usize, top: &[Edge], bottom: &[Edge]) -> Row {
        Row {
            lane,
            color,
            top: top.to_vec(),
            bottom: bottom.to_vec(),
            in_head_ancestry: true,
        }
    }

    fn text(cells: &[GlyphCell]) -> String {
        cells.iter().map(|c| c.ch).collect()
    }

    /// The character for a cell whose lines go the `ways` named: `u`p,
    /// `d`own, `l`eft, `r`ight.
    fn joint(ways: &str) -> char {
        box_char(&Joints {
            up: ways.contains('u'),
            down: ways.contains('d'),
            left: ways.contains('l'),
            right: ways.contains('r'),
            ..Joints::default()
        })
    }

    #[test]
    fn lanes_are_heavy_vertical_lines() {
        assert_eq!(joint("ud"), '┃');
        assert_eq!(joint("u"), '┃');
        assert_eq!(joint("d"), '┃');
    }

    #[test]
    fn junctions_keep_the_lane_heavy_and_the_branch_light() {
        assert_eq!(joint("udr"), '┠', "up, down, right");
        assert_eq!(joint("udl"), '┨', "up, down, left");
        assert_eq!(joint("udlr"), '╂', "every way");
        assert_eq!(joint("dlr"), '┰', "down, left, right");
        assert_eq!(joint("ulr"), '┸', "up, left, right");
    }

    #[test]
    fn horizontals_and_curves_stay_light_and_rounded() {
        assert_eq!(joint("lr"), '─');
        assert_eq!(joint("dr"), '╭');
        assert_eq!(joint("dl"), '╮');
        assert_eq!(joint("ur"), '╰');
        assert_eq!(joint("ul"), '╯');
        assert_eq!(joint(""), ' ');
    }

    #[test]
    fn a_curve_joining_a_lane_from_the_left() {
        let above = row(0, 0, &[e(1, 1, 1)], &[e(1, 1, 1), e(0, 1, 0)]);
        assert_eq!(text(&connector_glyphs(&above, None, 2)), "╰─┨");
    }

    #[test]
    fn single_commit_in_a_linear_history() {
        let r = row(0, 0, &[e(0, 0, 0)], &[e(0, 0, 0)]);
        assert_eq!(text(&lane_glyphs(&r, 1)), "●");
    }

    #[test]
    fn pass_through_lanes_are_vertical_lines() {
        let r = row(1, 1, &[e(0, 0, 0), e(1, 1, 1)], &[e(0, 0, 0), e(1, 1, 1)]);
        assert_eq!(text(&lane_glyphs(&r, 2)), "┃ ●");
    }

    #[test]
    fn commit_lines_have_no_horizontal_lines() {
        let merge = row(0, 0, &[], &[e(0, 0, 0), e(0, 1, 1)]);
        assert_eq!(text(&lane_glyphs(&merge, 2)), "●  ");
        let joined = row(0, 0, &[e(0, 0, 0), e(1, 0, 1)], &[]);
        assert_eq!(text(&lane_glyphs(&joined, 2)), "●  ");
    }

    #[test]
    fn trailing_columns_are_padded() {
        let r = row(0, 0, &[], &[e(0, 0, 0)]);
        assert_eq!(text(&lane_glyphs(&r, 3)), "●    ");
    }

    #[test]
    fn consecutive_commits_are_joined_by_a_line() {
        let above = row(0, 0, &[e(0, 0, 0)], &[e(0, 0, 0)]);
        let below = row(0, 0, &[e(0, 0, 0)], &[e(0, 0, 0)]);
        assert_eq!(text(&connector_glyphs(&above, Some(&below), 1)), "┃");
    }

    #[test]
    fn a_merge_curves_out_to_its_second_parent() {
        let merge = row(0, 0, &[], &[e(0, 0, 0), e(0, 1, 1)]);
        let next = row(0, 0, &[e(0, 0, 0), e(1, 1, 1)], &[e(0, 0, 0), e(1, 1, 1)]);
        assert_eq!(text(&connector_glyphs(&merge, Some(&next), 2)), "┠─╮");
    }

    #[test]
    fn a_branch_curves_into_its_parent() {
        let tip = row(1, 1, &[e(0, 0, 0)], &[e(0, 0, 0), e(1, 1, 1)]);
        let parent = row(0, 0, &[e(0, 0, 0), e(1, 0, 1)], &[e(0, 0, 0)]);
        assert_eq!(text(&connector_glyphs(&tip, Some(&parent), 2)), "┠─╯");
    }

    #[test]
    fn a_lane_moving_left_turns_with_rounded_corners() {
        let above = row(1, 1, &[e(1, 1, 1)], &[e(1, 0, 1)]);
        let below = row(0, 1, &[e(0, 0, 1)], &[]);
        assert_eq!(text(&connector_glyphs(&above, Some(&below), 2)), "╭─╯");
    }

    #[test]
    fn the_last_loaded_row_continues_down() {
        let r = row(0, 0, &[], &[e(0, 0, 0), e(0, 1, 1)]);
        assert_eq!(text(&connector_glyphs(&r, None, 2)), "┠─╮");
    }

    #[test]
    fn a_curve_crossing_a_lane() {
        let above = row(0, 0, &[e(1, 1, 4)], &[e(0, 0, 0), e(1, 1, 4), e(0, 2, 9)]);
        let cells = connector_glyphs(&above, None, 3);
        assert_eq!(text(&cells), "┠─╂─╮");
        assert_eq!(cells[2].color, Some(4), "the crossed lane keeps its color");
        assert_eq!(
            cells[4].color,
            Some(9),
            "the curve has the new lane's color"
        );
    }

    #[test]
    fn straight_lanes_keep_their_color_where_a_curve_leaves() {
        let merge = row(0, 0, &[], &[e(0, 0, 0), e(0, 1, 7)]);
        let colors: Vec<Option<usize>> = connector_glyphs(&merge, None, 2)
            .iter()
            .map(|c| c.color)
            .collect();
        assert_eq!(
            colors,
            [Some(0), Some(7), Some(7)],
            "lane, horizontal, corner"
        );
    }
}
