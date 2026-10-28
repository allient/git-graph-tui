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

/// Draws one row's lanes as box-drawing characters, `width` lanes wide.
///
/// Lane `i` is cell `2 * i`; the odd cells between lanes only carry
/// horizontal lines. The result has `2 * width - 1` cells.
#[must_use]
pub fn lane_glyphs(row: &Row, width: usize) -> Vec<GlyphCell> {
    let width = width.max(row.width()).max(1);
    let mut joints = vec![Joints::default(); 2 * width - 1];

    // Top half: lines arrive from above at `from` and run sideways to `to`.
    for edge in &row.top {
        let cell = &mut joints[2 * edge.from];
        cell.up = true;
        cell.vertical = Some(edge.color);
        horizontal(&mut joints, edge.from, edge);
    }
    // Bottom half: lines leave from `from` sideways, then go down at `to`.
    for edge in &row.bottom {
        let cell = &mut joints[2 * edge.to];
        cell.down = true;
        cell.vertical = Some(edge.color);
        horizontal(&mut joints, edge.to, edge);
    }

    joints
        .iter()
        .enumerate()
        .map(|(i, j)| {
            if i == 2 * row.lane {
                GlyphCell {
                    ch: '●',
                    color: Some(row.color),
                }
            } else {
                GlyphCell {
                    ch: box_char(j),
                    color: j.vertical.or(j.horizontal).filter(|_| box_char(j) != ' '),
                }
            }
        })
        .collect()
}

/// Adds the sideways part of `edge`, which turns vertical at lane `corner`.
fn horizontal(joints: &mut [Joints], corner: usize, edge: &Edge) {
    if edge.from == edge.to {
        return;
    }
    let other = if corner == edge.from {
        edge.to
    } else {
        edge.from
    };
    let (lo, hi) = (2 * corner.min(other), 2 * corner.max(other));
    if other > corner {
        joints[lo].right = true;
    } else {
        joints[hi].left = true;
    }
    for cell in &mut joints[lo + 1..hi] {
        cell.left = true;
        cell.right = true;
        cell.horizontal.get_or_insert(edge.color);
    }
}

fn box_char(j: &Joints) -> char {
    match (j.up, j.down, j.left, j.right) {
        (true, true, true, true) => '┼',
        (true, true, true, false) => '┤',
        (true, true, false, true) => '├',
        (true, false, true, true) => '┴',
        (false, true, true, true) => '┬',
        (true, false, true, false) => '╯',
        (true, false, false, true) => '╰',
        (false, true, true, false) => '╮',
        (false, true, false, true) => '╭',
        (true, _, false, false) | (false, true, false, false) => '│',
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

    #[test]
    fn single_commit_in_a_linear_history() {
        let r = row(0, 0, &[e(0, 0, 0)], &[e(0, 0, 0)]);
        assert_eq!(text(&lane_glyphs(&r, 1)), "●");
    }

    #[test]
    fn pass_through_lanes_are_vertical_lines() {
        let r = row(1, 1, &[e(0, 0, 0), e(1, 1, 1)], &[e(0, 0, 0), e(1, 1, 1)]);
        assert_eq!(text(&lane_glyphs(&r, 2)), "│ ●");
    }

    #[test]
    fn merge_opens_a_lane_to_the_right() {
        let r = row(0, 0, &[], &[e(0, 0, 0), e(0, 1, 1)]);
        assert_eq!(text(&lane_glyphs(&r, 2)), "●─╮");
    }

    #[test]
    fn lane_from_the_right_joins_the_node() {
        let r = row(0, 0, &[e(0, 0, 0), e(1, 0, 1)], &[]);
        assert_eq!(text(&lane_glyphs(&r, 2)), "●─╯");
    }

    #[test]
    fn lane_from_the_left_joins_the_node() {
        let r = row(1, 1, &[e(0, 1, 0), e(1, 1, 1)], &[e(1, 1, 1)]);
        assert_eq!(text(&lane_glyphs(&r, 2)), "╰─●");
    }

    #[test]
    fn branch_off_to_the_left() {
        let r = row(1, 1, &[e(1, 1, 1)], &[e(1, 0, 2), e(1, 1, 1)]);
        assert_eq!(text(&lane_glyphs(&r, 2)), "╭─●");
    }

    #[test]
    fn horizontal_line_crossing_a_lane() {
        let r = row(0, 0, &[e(1, 1, 1)], &[e(0, 0, 0), e(1, 1, 1), e(0, 2, 2)]);
        assert_eq!(text(&lane_glyphs(&r, 3)), "●─┼─╮");
    }

    #[test]
    fn trailing_columns_are_padded() {
        let r = row(0, 0, &[], &[e(0, 0, 0)]);
        assert_eq!(text(&lane_glyphs(&r, 3)), "●    ");
    }

    #[test]
    fn colors_follow_the_segments() {
        let r = row(0, 0, &[], &[e(0, 0, 0), e(0, 1, 7)]);
        let colors: Vec<Option<usize>> = lane_glyphs(&r, 2).iter().map(|c| c.color).collect();
        assert_eq!(
            colors,
            [Some(0), Some(7), Some(7)],
            "node, horizontal, corner"
        );
    }

    #[test]
    fn vertical_lines_keep_their_color_where_lines_cross() {
        let r = row(0, 0, &[e(1, 1, 4)], &[e(0, 0, 0), e(1, 1, 4), e(0, 2, 9)]);
        let cells = lane_glyphs(&r, 3);
        assert_eq!(cells[2].ch, '┼');
        assert_eq!(cells[2].color, Some(4));
    }
}
