use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tgg_git::show::{FileChange, Numstat, parse_diff};

use super::*;
use crate::{Key, Theme};

fn file() -> FileChange {
    FileChange {
        path: "src/lib.rs".into(),
        old_path: None,
        status: tgg_model::Change::Modified,
        stat: Numstat::Lines {
            added: 1,
            deleted: 1,
        },
    }
}

const PATCH: &str = "diff --git a/src/lib.rs b/src/lib.rs\n\
                     index 1111111..2222222 100644\n\
                     --- a/src/lib.rs\n\
                     +++ b/src/lib.rs\n\
                     @@ -1,3 +1,3 @@\n \
                     fn a() {}\n\
                     -fn b() {}\n\
                     +fn b() -> u8 {\t1 }\n \
                     fn c() {}\n";

fn view() -> DiffView {
    DiffView::new(&file(), parse_diff(PATCH))
}

/// 20 context lines.
fn long_view() -> DiffView {
    let body = (1..=20)
        .map(|i| format!(" line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    DiffView::new(&file(), parse_diff(&format!("@@ -1,20 +1,20 @@\n{body}\n")))
}

fn render(view: &DiffView, area: Rect) -> (Buffer, DiffHitMap) {
    let mut buf = Buffer::empty(area);
    let hits = view.render(area, &mut buf, &Theme::default());
    (buf, hits)
}

fn row_text(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width)
        .map(|x| buf[(x, y)].symbol())
        .collect::<String>()
        .trim_end()
        .to_owned()
}

#[test]
fn scrolling_is_clamped_to_the_content() {
    let mut view = long_view();
    // 21 lines (hunk + 20), 5 visible.
    let page = 5;
    view.handle_key(Key::Down, page);
    assert_eq!(view.offset(), 1);
    view.handle_key(Key::PageDown, page);
    assert_eq!(view.offset(), 6);
    view.handle_key(Key::End, page);
    assert_eq!(view.offset(), 16, "last line at the bottom");
    view.handle_key(Key::Down, page);
    assert_eq!(view.offset(), 16);
    view.handle_key(Key::PageUp, page);
    assert_eq!(view.offset(), 11);
    view.handle_key(Key::Home, page);
    assert_eq!(view.offset(), 0);
    view.handle_key(Key::Up, page);
    assert_eq!(view.offset(), 0);
}

#[test]
fn mouse_wheel_scrolls_by_a_delta() {
    let mut view = long_view();
    view.scroll_by(3, 5);
    assert_eq!(view.offset(), 3);
    view.scroll_by(-10, 5);
    assert_eq!(view.offset(), 0);
    view.scroll_by(100, 5);
    assert_eq!(view.offset(), 16);
}

#[test]
fn a_diff_shorter_than_the_page_does_not_scroll() {
    let mut view = view();
    view.handle_key(Key::End, 50);
    assert_eq!(view.offset(), 0);
}

#[test]
fn escape_and_q_close() {
    let mut view = view();
    assert_eq!(view.handle_key(Key::Esc, 5), DiffOutcome::Close);
    assert_eq!(view.handle_key(Key::Char('q'), 5), DiffOutcome::Close);
    assert_eq!(view.handle_key(Key::Down, 5), DiffOutcome::Stay);
}

#[test]
fn header_shows_the_path_and_counts() {
    let (buf, _) = render(&view(), Rect::new(0, 0, 60, 12));
    assert_eq!(row_text(&buf, 0), " src/lib.rs (+1 -1)");
}

#[test]
fn lines_have_a_line_number_gutter_and_tabs_expand() {
    let (buf, _) = render(&view(), Rect::new(0, 0, 60, 12));
    assert_eq!(row_text(&buf, 6), "    1    1 │  fn a() {}");
    assert_eq!(row_text(&buf, 7), "    2      │ -fn b() {}");
    assert_eq!(row_text(&buf, 8), "         2 │ +fn b() -> u8 {    1 }");
}

#[test]
fn added_and_deleted_lines_are_colored() {
    let style = DiffStyle::default();
    let (buf, _) = render(&view(), Rect::new(0, 0, 60, 12));
    assert_eq!(buf[(14, 7)].fg, style.deleted);
    assert_eq!(buf[(14, 8)].fg, style.added);
    assert_eq!(buf[(14, 5)].fg, style.hunk);
}

#[test]
fn hit_map_maps_rows_to_diff_lines() {
    let mut view = long_view();
    view.scroll_by(4, 5);
    let (_, hits) = render(&view, Rect::new(0, 10, 40, 6));
    assert_eq!(hits.hit(3, 10), DiffHit::Header);
    assert_eq!(hits.hit(3, 11), DiffHit::Line(4));
    assert_eq!(hits.hit(3, 15), DiffHit::Line(8));
    assert_eq!(hits.hit(3, 0), DiffHit::Outside);
}

#[test]
fn binary_files_say_so() {
    let view = DiffView::new(
        &FileChange {
            path: "logo.png".into(),
            old_path: None,
            status: tgg_model::Change::Modified,
            stat: Numstat::Binary,
        },
        parse_diff(
            "diff --git a/logo.png b/logo.png\nBinary files a/logo.png and b/logo.png differ\n",
        ),
    );
    let (buf, _) = render(&view, Rect::new(0, 0, 60, 6));
    assert_eq!(row_text(&buf, 0), " logo.png (binary)");
    assert_eq!(row_text(&buf, 1), " Binary file, no text diff to show.");
}

/// Two deletions replaced by three additions, then context.
const RUNS: &str = "diff --git a/a.txt b/a.txt\n\
                    index 1111111..2222222 100644\n\
                    --- a/a.txt\n\
                    +++ b/a.txt\n\
                    @@ -1,3 +1,4 @@\n\
                    -one\n\
                    -two\n\
                    +uno\n\
                    +dos\n\
                    +tres\n \
                    same\n";

fn wide() -> Rect {
    Rect::new(0, 0, SIDE_BY_SIDE_WIDTH, 10)
}

#[test]
fn side_by_side_pairs_runs_and_leaves_blanks() {
    let view =
        DiffView::titled("a.txt (working tree)".into(), parse_diff(RUNS)).with_side_by_side(true);
    let (buf, hits) = render(&view, wide());
    assert_eq!(row_text(&buf, 0), " a.txt (working tree) (+3 -2)");
    // The patch header is left out; the hunk spans the width.
    assert_eq!(row_text(&buf, 1), " @@ -1,3 +1,4 @@");
    let half = usize::from(SIDE_BY_SIDE_WIDTH / 2);
    let row = |y| {
        let text: Vec<String> = (0..buf.area.width)
            .map(|x| buf[(x, y)].symbol().to_owned())
            .collect();
        let left = text[..half].concat().trim_end().to_owned();
        let right = text[half + 1..].concat().trim_end().to_owned();
        (left, text[half].clone(), right)
    };
    assert_eq!(row(2), ("   1 one".into(), "│".into(), "   1 uno".into()));
    assert_eq!(row(3), ("   2 two".into(), "│".into(), "   2 dos".into()));
    assert_eq!(row(4), (String::new(), "│".into(), "   3 tres".into()));
    assert_eq!(row(5), ("   3 same".into(), "│".into(), "   4 same".into()));
    let style = DiffStyle::default();
    assert_eq!(buf[(5, 2)].fg, style.deleted);
    assert_eq!(buf[(u16::try_from(half).unwrap() + 6, 2)].fg, style.added);
    assert_eq!(
        hits.hit(3, 4),
        DiffHit::Line(9),
        "the added line of a blank-left row"
    );
}

#[test]
fn side_by_side_falls_back_to_unified_when_narrow() {
    let view = DiffView::titled("a.txt".into(), parse_diff(RUNS)).with_side_by_side(true);
    let (buf, hits) = render(&view, Rect::new(0, 0, SIDE_BY_SIDE_WIDTH - 1, 12));
    assert_eq!(hits.width(), SIDE_BY_SIDE_WIDTH - 1);
    assert_eq!(row_text(&buf, 6), "    1      │ -one");
    assert!(view.side_by_side(), "the choice is kept for a wider area");
}

#[test]
fn side_by_side_scrolls_by_rows() {
    let mut view = DiffView::titled("a.txt".into(), parse_diff(RUNS)).with_side_by_side(true);
    render(&view, wide());
    // Hunk, three pairs (two deletions against three additions), context.
    view.handle_key(Key::End, 2);
    assert_eq!(view.offset(), 3);
}

#[test]
fn a_plain_file_never_goes_side_by_side() {
    let mut view = DiffView::text("a.rs @ abc".into(), "x\n");
    assert!(!view.can_split());
    view.set_side_by_side(true);
    assert!(!view.side_by_side());
}

#[test]
fn a_multi_file_view_stacks_each_file_under_its_header() {
    let other = "diff --git a/b.txt b/b.txt\n\
                 --- a/b.txt\n\
                 +++ b/b.txt\n\
                 @@ -1 +1 @@\n\
                 -old\n\
                 +new\n";
    let view = DiffView::multi(
        "Changes (working tree)".into(),
        vec![
            ("a.txt M".into(), parse_diff(RUNS)),
            ("b.txt M".into(), parse_diff(other)),
        ],
    );
    let (buf, _) = render(&view, Rect::new(0, 0, 60, 20));
    assert_eq!(row_text(&buf, 0), " Changes (working tree) (+4 -3)");
    assert_eq!(row_text(&buf, 1), " ▾ a.txt M");
    assert_eq!(buf[(30, 1)].bg, Theme::default().header_bg);
    assert_eq!(row_text(&buf, 7), "    1      │ -one");
    assert_eq!(row_text(&buf, 13), " ▾ b.txt M");
    assert_eq!(row_text(&buf, 18), "    1      │ -old");
    assert_eq!(row_text(&buf, 19), "         1 │ +new");

    let split = view.with_side_by_side(true);
    let (buf, _) = render(&split, Rect::new(0, 0, SIDE_BY_SIDE_WIDTH, 20));
    assert_eq!(row_text(&buf, 1), " ▾ a.txt M");
    assert_eq!(row_text(&buf, 7), " ▾ b.txt M");
    assert_eq!(row_text(&buf, 8), " @@ -1 +1 @@");
}
