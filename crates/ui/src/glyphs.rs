use tgg_graph::{Edge, Row};

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
/// lanes wide.
///
/// Lane `i` is cell `2 * i`; the cells between lanes carry horizontal lines.
/// As in VS Code's Git Graph, a line between a commit and another lane turns
/// once, here on the commit's line: a fork leaves the node sideways and turns
/// down its new lane (`●─╮`), and a branch runs straight down its own lane
/// and turns into the node it joins (`●─╯`). The result has `2 * width - 1`
/// cells.
#[must_use]
pub fn lane_glyphs(row: &Row, width: usize) -> Vec<GlyphCell> {
    let width = width.max(row.width()).max(1);
    let mut joints = vec![Joints::default(); 2 * width - 1];
    // (cell the line runs vertically in, the cell it turns to, goes up, color)
    let mut ends: Vec<(usize, usize, bool, usize)> = row
        .top
        .iter()
        .map(|e| (entry(row, e), 2 * e.to, true, e.color))
        .chain(
            row.bottom
                .iter()
                .map(|e| (exit(row, e), 2 * e.from, false, e.color)),
        )
        .collect();
    // Straight lanes first, so a bend leaving a lane doesn't recolor it.
    ends.sort_by_key(|&(x, lane, _, _)| x != lane);
    for (x, lane, up, color) in ends {
        let cell = &mut joints[x];
        if up {
            cell.up = true;
        } else {
            cell.down = true;
        }
        cell.vertical.get_or_insert(color);
        span(&mut joints, x, lane, color);
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
/// Each line leaving `above` runs down to where it enters `below`. Lines to
/// and from a commit already turned on the commit lines, so they run
/// straight here; only a passing lane that moves to another lane crosses
/// over, with rounded corners at both ends (`╭─╯`).
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
        let top = &mut joints[upper];
        top.up = true;
        top.vertical.get_or_insert(color);
        let bottom = &mut joints[lower];
        bottom.down = true;
        bottom.vertical.get_or_insert(color);
        span(&mut joints, upper, lower, color);
    }
    render(&joints)
}

/// The lines crossing the boundary between two rows, as (cell above, cell
/// below, color). A line starts where it left the commit line above (see
/// [`exit`]) and ends where it enters the commit line below (see [`entry`]).
/// The color is the one of the part that changes lanes.
fn segments(above: &Row, below: Option<&Row>) -> Vec<(usize, usize, usize)> {
    let Some(below) = below else {
        // The last row loaded: every line just continues down.
        return above
            .bottom
            .iter()
            .map(|out| (exit(above, out), 2 * out.to, out.color))
            .collect();
    };
    let mut segments = Vec::new();
    for out in &above.bottom {
        let upper = exit(above, out);
        let mut joined = below.top.iter().filter(|e| e.from == out.to).peekable();
        if joined.peek().is_none() {
            segments.push((upper, 2 * out.to, out.color));
        }
        for e in joined {
            let color = if out.from == out.to {
                e.color
            } else {
                out.color
            };
            segments.push((upper, entry(below, e), color));
        }
    }
    // Lines entering `below` that nothing above leads to.
    for e in &below.top {
        if !above.bottom.iter().any(|out| out.to == e.from) {
            segments.push((2 * e.from, entry(below, e), e.color));
        }
    }
    segments
}

/// The cell where edge `e` from above enters `row`'s commit line: the lane
/// it comes down, so a branch joining the commit turns into it here.
fn entry(row: &Row, e: &Edge) -> usize {
    if e.to == row.lane {
        2 * e.from
    } else {
        2 * e.to
    }
}

/// The cell where edge `e` leaves `row`'s commit line downwards: the lane it
/// goes to when it forks off the commit, so it turned here already.
fn exit(row: &Row, e: &Edge) -> usize {
    if e.from == row.lane {
        2 * e.to
    } else {
        2 * e.from
    }
}

/// Adds a horizontal line between cells `a` and `b`.
fn span(joints: &mut [Joints], a: usize, b: usize, color: usize) {
    if a == b {
        return;
    }
    let (lo, hi) = (a.min(b), a.max(b));
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
    fn a_line_joining_a_lane_from_the_left_turns_on_the_commit_line() {
        let above = row(0, 0, &[e(1, 1, 1)], &[e(1, 1, 1), e(0, 1, 0)]);
        assert_eq!(text(&lane_glyphs(&above, 2)), "●─┨");
        assert_eq!(text(&connector_glyphs(&above, None, 2)), "  ┃");
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
    fn lines_turn_once_on_the_commit_line() {
        let merge = row(0, 0, &[], &[e(0, 0, 0), e(0, 1, 1)]);
        assert_eq!(text(&lane_glyphs(&merge, 2)), "●─╮");
        let joined = row(0, 0, &[e(0, 0, 0), e(1, 0, 1)], &[]);
        assert_eq!(text(&lane_glyphs(&joined, 2)), "●─╯");
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
    fn a_fork_to_the_right_turns_at_the_commit_then_runs_straight() {
        let merge = row(0, 0, &[], &[e(0, 0, 0), e(0, 1, 1)]);
        let next = row(1, 1, &[e(0, 0, 0), e(1, 1, 1)], &[e(0, 0, 0), e(1, 1, 1)]);
        assert_eq!(text(&lane_glyphs(&merge, 2)), "●─╮");
        assert_eq!(text(&connector_glyphs(&merge, Some(&next), 2)), "┃ ┃");
        assert_eq!(text(&lane_glyphs(&next, 2)), "┃ ●");
    }

    #[test]
    fn a_branch_joining_from_the_right_runs_straight_then_turns_into_its_parent() {
        let tip = row(1, 1, &[e(0, 0, 0)], &[e(0, 0, 0), e(1, 1, 1)]);
        let parent = row(0, 0, &[e(0, 0, 0), e(1, 0, 1)], &[e(0, 0, 0)]);
        assert_eq!(text(&lane_glyphs(&tip, 2)), "┃ ●");
        assert_eq!(text(&connector_glyphs(&tip, Some(&parent), 2)), "┃ ┃");
        assert_eq!(text(&lane_glyphs(&parent, 2)), "●─╯");
    }

    #[test]
    fn a_node_moving_left_turns_on_its_own_line() {
        let above = row(1, 1, &[e(1, 1, 1)], &[e(1, 0, 1)]);
        let below = row(0, 1, &[e(0, 0, 1)], &[]);
        assert_eq!(text(&lane_glyphs(&above, 2)), "╭─●");
        assert_eq!(text(&connector_glyphs(&above, Some(&below), 2)), "┃  ");
    }

    #[test]
    fn a_passing_lane_moving_left_by_one() {
        let above = row(0, 0, &[e(0, 0, 0), e(2, 2, 2)], &[e(0, 0, 0), e(2, 1, 2)]);
        let below = row(0, 0, &[e(0, 0, 0), e(1, 1, 2)], &[e(0, 0, 0), e(1, 1, 2)]);
        assert_eq!(text(&lane_glyphs(&above, 3)), "●   ┃");
        assert_eq!(text(&connector_glyphs(&above, Some(&below), 3)), "┃ ╭─╯");
        assert_eq!(text(&lane_glyphs(&below, 3)), "● ┃  ");
    }

    #[test]
    fn a_passing_lane_arriving_from_the_right() {
        let above = row(1, 1, &[e(1, 1, 1), e(2, 2, 2)], &[e(1, 1, 1), e(2, 2, 2)]);
        let passing = row(1, 1, &[e(1, 1, 1), e(2, 0, 2)], &[e(0, 0, 2), e(1, 1, 1)]);
        assert_eq!(text(&connector_glyphs(&above, Some(&passing), 3)), "╭─╂─╯");
        assert_eq!(text(&lane_glyphs(&passing, 3)), "┃ ●  ");
    }

    #[test]
    fn a_long_fork_turns_at_the_commit() {
        let fork = row(0, 0, &[], &[e(0, 0, 0), e(0, 3, 3)]);
        let next = row(3, 3, &[e(0, 0, 0), e(3, 3, 3)], &[e(0, 0, 0), e(3, 3, 3)]);
        assert_eq!(text(&lane_glyphs(&fork, 4)), "●─────╮");
        assert_eq!(text(&connector_glyphs(&fork, Some(&next), 4)), "┃     ┃");
    }

    #[test]
    fn the_last_loaded_row_continues_down() {
        let r = row(0, 0, &[], &[e(0, 0, 0), e(0, 1, 1)]);
        assert_eq!(text(&connector_glyphs(&r, None, 2)), "┃ ┃");
    }

    #[test]
    fn a_line_crossing_a_lane_on_the_commit_line() {
        let above = row(0, 0, &[e(1, 1, 4)], &[e(0, 0, 0), e(1, 1, 4), e(0, 2, 9)]);
        let cells = lane_glyphs(&above, 3);
        assert_eq!(text(&cells), "●─╂─╮");
        assert_eq!(cells[2].color, Some(4), "the crossed lane keeps its color");
        assert_eq!(cells[4].color, Some(9), "the turn has the new lane's color");
        assert_eq!(text(&connector_glyphs(&above, None, 3)), "┃ ┃ ┃");
    }

    #[test]
    fn lines_arriving_and_leaving_on_one_side_combine() {
        let r = row(0, 0, &[e(1, 0, 1)], &[e(0, 1, 2)]);
        assert_eq!(text(&lane_glyphs(&r, 2)), "●─┨");
    }

    #[test]
    fn straight_lanes_keep_their_color_where_a_line_turns_off() {
        let merge = row(0, 0, &[], &[e(0, 0, 0), e(0, 1, 7)]);
        let line: Vec<Option<usize>> = lane_glyphs(&merge, 2).iter().map(|c| c.color).collect();
        assert_eq!(line, [Some(0), Some(7), Some(7)], "node, the turn's color");
        let colors: Vec<Option<usize>> = connector_glyphs(&merge, None, 2)
            .iter()
            .map(|c| c.color)
            .collect();
        assert_eq!(colors, [Some(0), None, Some(7)], "both lanes, straight");
    }
}
