//! Renders the toolbar and the ref picker into a buffer.

use ggt_ui::{Key, Picker, PickerItem, Theme, Toolbar};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};

fn item(value: &str, kind: &str) -> PickerItem {
    PickerItem {
        value: value.into(),
        kind: Some(kind.into()),
        search_kind: false,
    }
}

fn refs() -> Vec<PickerItem> {
    vec![
        item("main", "branch"),
        item("develop", "branch"),
        item("feature/login", "branch"),
        item("origin/main", "remote"),
        item("v0.1.0", "tag"),
    ]
}

fn text(buf: &Buffer) -> String {
    let area = buf.area;
    (area.top()..area.bottom())
        .map(|y| {
            (area.left()..area.right())
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn checkout_picker_filtered() {
    let mut picker = Picker::single("Checkout to", refs());
    for ch in "ma".chars() {
        picker.handle_key(Key::Char(ch));
    }
    let area = Rect::new(0, 0, 64, 10);
    let mut buf = Buffer::empty(area);
    picker.render(area, &mut buf, &Theme::default());
    insta::assert_snapshot!(text(&buf));
}

#[test]
fn branch_filter_with_a_typed_pattern_below_the_toolbar() {
    let area = Rect::new(0, 0, 80, 12);
    let mut buf = Buffer::empty(area);
    let toolbar = Toolbar::new(&["develop".to_owned()], true);
    let hits = toolbar.render(Rect::new(0, 0, 80, 1), &mut buf, &Theme::default());
    let below = hits
        .button(ggt_ui::ToolbarButton::BranchFilter)
        .map(|r| Position::new(r.x, r.y + 1))
        .unwrap_or_default();
    let mut picker = Picker::multi("Branches", refs(), &["develop".to_owned()]).with_anchor(below);
    for ch in "feat*".chars() {
        picker.handle_key(Key::Char(ch));
    }
    picker.render(area, &mut buf, &Theme::default());
    insta::assert_snapshot!(text(&buf));
}
