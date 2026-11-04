use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::*;

fn modes() -> Select {
    Select::new(
        vec![
            SelectOption {
                value: "soft".into(),
                description: "Keep all changes, but reset HEAD".into(),
                destructive: false,
            },
            SelectOption {
                value: "mixed".into(),
                description: "Keep working tree, but reset index".into(),
                destructive: false,
            },
            SelectOption {
                value: "hard".into(),
                description: "Discard all changes".into(),
                destructive: true,
            },
        ],
        "mixed",
    )
}

fn text(buf: &Buffer) -> String {
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn starts_on_the_given_value_and_keeps_an_unknown_one() {
    assert_eq!(modes().value(), "mixed");
    let select = Select::new(vec![SelectOption::plain("origin")], "upstream");
    assert_eq!(select.value(), "upstream", "never lost");
}

#[test]
fn closed_it_leaves_keys_it_does_not_use_to_its_host() {
    let mut select = modes();
    for key in [
        Key::Enter,
        Key::Tab,
        Key::Esc,
        Key::Up,
        Key::Down,
        Key::Char('x'),
    ] {
        assert_eq!(select.handle_key(key), SelectEvent::Ignored, "{key:?}");
    }
    assert_eq!(select.handle_key(Key::Right), SelectEvent::Changed);
    assert_eq!(select.value(), "hard");
    assert_eq!(select.handle_key(Key::Right), SelectEvent::Changed);
    assert_eq!(select.value(), "soft", "wraps");
    assert_eq!(select.handle_key(Key::Left), SelectEvent::Changed);
    assert_eq!(select.value(), "hard");
}

#[test]
fn open_it_keeps_every_key_and_picks_with_enter() {
    let mut select = modes();
    assert_eq!(select.handle_key(Key::Char(' ')), SelectEvent::Handled);
    assert!(select.is_open());
    assert_eq!(select.handle_key(Key::Char('x')), SelectEvent::Handled);
    assert_eq!(select.handle_key(Key::Down), SelectEvent::Handled);
    assert_eq!(select.value(), "mixed", "moving only highlights");
    assert_eq!(select.handle_key(Key::Enter), SelectEvent::Changed);
    assert!(!select.is_open());
    assert_eq!(select.value(), "hard");
}

#[test]
fn the_highlight_stops_at_both_ends() {
    let mut select = modes();
    select.open();
    for _ in 0..5 {
        select.handle_key(Key::Down);
    }
    select.handle_key(Key::Enter);
    assert_eq!(select.value(), "hard");
    select.open();
    for _ in 0..5 {
        select.handle_key(Key::Up);
    }
    select.handle_key(Key::Enter);
    assert_eq!(select.value(), "soft");
}

#[test]
fn esc_closes_without_changing_the_value() {
    let mut select = modes();
    select.open();
    select.handle_key(Key::Down);
    assert_eq!(select.handle_key(Key::Esc), SelectEvent::Handled);
    assert!(!select.is_open());
    assert_eq!(select.value(), "mixed");
}

#[test]
fn picking_the_current_option_changes_nothing() {
    let mut select = modes();
    select.open();
    assert_eq!(select.handle_key(Key::Enter), SelectEvent::Handled);
    assert_eq!(select.value(), "mixed");
}

#[test]
fn the_button_and_the_list_look_like_a_dropdown_menu() {
    let screen = Rect::new(0, 0, 60, 8);
    let mut buf = Buffer::empty(screen);
    let mut select = modes();
    select.open();
    let button = Rect::new(2, 1, 56, 1);
    let theme = Theme::default();
    select.render_button(&mut buf, button, Style::default(), &theme, Color::Red);
    let hits = select.render_list(&mut buf, button, screen, &theme, Color::Red);
    assert_eq!(
        text(&buf),
        [
            "",
            "   mixed ▴   Keep working tree, but reset index",
            " ┌──────────────────────────────────────────────┐",
            " │   soft      Keep all changes, but reset HEAD │",
            " │ ✓ mixed   Keep working tree, but reset index │",
            " │   hard                   Discard all changes │",
            " └──────────────────────────────────────────────┘",
            "",
        ]
        .join("\n")
    );
    assert_eq!(hits.hit(10, 5), SelectHit::Option(2));
    assert_eq!(hits.hit(1, 3), SelectHit::List, "the frame");
    assert_eq!(hits.hit(10, 0), SelectHit::Outside);
    let x = (0..60).find(|&x| buf[(x, 5)].symbol() == "D").unwrap();
    assert_eq!(
        buf[(x, 5)].fg,
        Color::Red,
        "destructive in the warning color"
    );
}

#[test]
fn a_click_picks_an_option_and_anywhere_else_only_closes() {
    let screen = Rect::new(0, 0, 60, 8);
    let mut buf = Buffer::empty(screen);
    let button = Rect::new(2, 1, 56, 1);
    let mut select = modes();
    select.open();
    let hits = select.render_list(&mut buf, button, screen, &Theme::default(), Color::Red);
    assert_eq!(select.handle_click(&hits, 50, 0), SelectEvent::Handled);
    assert!(!select.is_open());
    assert_eq!(select.value(), "mixed");
    select.open();
    assert_eq!(select.handle_click(&hits, 10, 3), SelectEvent::Changed);
    assert_eq!(select.value(), "soft");
    assert_eq!(
        select.handle_click(&hits, 10, 3),
        SelectEvent::Ignored,
        "closed"
    );
}

#[test]
fn the_list_goes_under_the_button_else_above_it_never_over_it() {
    let screen = Rect::new(0, 0, 80, 20);
    let button = |y| Rect::new(10, y, 40, 1);
    assert_eq!(list_top(button(5), 5, screen), 6, "room below");
    assert_eq!(list_top(button(17), 5, screen), 12, "room above only");
    let tiny = Rect::new(0, 0, 80, 6);
    assert_eq!(
        list_top(button(3), 5, tiny),
        4,
        "no room: under it, cut off"
    );
}
