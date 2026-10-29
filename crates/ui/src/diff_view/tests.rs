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
