use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};

use super::*;
use crate::{Key, Theme};

fn item(value: &str, kind: &str) -> PickerItem {
    PickerItem {
        value: value.into(),
        kind: Some(kind.into()),
    }
}

fn refs() -> Vec<PickerItem> {
    vec![
        item("main", "branch"),
        item("develop", "branch"),
        item("feature/login", "branch"),
        item("feature/search", "branch"),
        item("origin/main", "remote"),
        item("v1.0", "tag"),
    ]
}

fn single() -> Picker {
    Picker::single("Checkout to", refs())
}

fn multi(selected: &[&str]) -> Picker {
    let selected: Vec<String> = selected.iter().map(|s| (*s).to_owned()).collect();
    Picker::multi("Branches", refs(), &selected)
}

fn type_text(picker: &mut Picker, text: &str) {
    for ch in text.chars() {
        picker.handle_key(Key::Char(ch));
    }
}

fn screen() -> Rect {
    Rect::new(0, 0, 80, 20)
}

fn render(picker: &Picker) -> (Buffer, PickerHitMap) {
    let mut buf = Buffer::empty(screen());
    let hits = picker.render(screen(), &mut buf, &Theme::default());
    (buf, hits)
}

// ---- globs ----

#[test]
fn glob_patterns_match_like_branch_filters() {
    assert!(glob_match("feature/*", "feature/login"));
    assert!(glob_match("feature/*", "feature/a/b"), "* crosses slashes");
    assert!(!glob_match("feature/*", "features/x"));
    assert!(glob_match("v?.0", "v1.0"));
    assert!(!glob_match("v?.0", "v10.0"));
    assert!(glob_match("*", ""));
    assert!(glob_match("*login*", "feature/login-page"));
    assert!(glob_match("main", "main"));
    assert!(!glob_match("main", "main2"));
}

// ---- single choice ----

#[test]
fn typing_filters_by_substring_ignoring_case() {
    let mut picker = single();
    type_text(&mut picker, "FEAT");
    assert_eq!(picker.visible_values(), ["feature/login", "feature/search"]);
    picker.handle_key(Key::Backspace);
    picker.handle_key(Key::Backspace);
    picker.handle_key(Key::Backspace);
    picker.handle_key(Key::Backspace);
    assert_eq!(picker.visible_values().len(), 6);
}

#[test]
fn a_typed_glob_filters_too() {
    let mut picker = single();
    type_text(&mut picker, "*/main");
    assert_eq!(picker.visible_values(), ["origin/main"]);
}

#[test]
fn enter_picks_the_highlighted_ref() {
    let mut picker = single();
    picker.handle_key(Key::Down);
    picker.handle_key(Key::Down);
    assert_eq!(
        picker.handle_key(Key::Enter),
        PickerOutcome::Picked("feature/login".into())
    );
    type_text(&mut picker, "zzz");
    assert_eq!(
        picker.handle_key(Key::Enter),
        PickerOutcome::Pending,
        "nothing to pick"
    );
    assert_eq!(picker.handle_key(Key::Esc), PickerOutcome::Cancelled);
}

#[test]
fn the_cursor_stays_on_a_visible_row_and_resets_when_filtering() {
    let mut picker = single();
    picker.handle_key(Key::Up);
    assert_eq!(picker.cursor(), 0, "no wrap above the first row");
    for _ in 0..10 {
        picker.handle_key(Key::Down);
    }
    assert_eq!(picker.cursor(), 5);
    type_text(&mut picker, "v");
    assert_eq!(picker.cursor(), 0);
    picker.handle_key(Key::PageDown);
    assert_eq!(picker.cursor(), picker.visible_values().len() - 1);
    picker.handle_key(Key::PageUp);
    assert_eq!(picker.cursor(), 0);
}

#[test]
fn spaces_are_typed_in_single_mode() {
    let mut picker = single();
    type_text(&mut picker, "a b");
    assert_eq!(picker.query(), "a b");
}

// ---- multi-select (branch filter) ----

#[test]
fn multi_mode_starts_with_show_all_and_the_current_selection() {
    let picker = multi(&["develop"]);
    assert_eq!(picker.rows()[0], Row::ShowAll);
    assert_eq!(picker.selected(), ["develop"]);
}

#[test]
fn space_toggles_and_enter_applies_the_selection() {
    let mut picker = multi(&[]);
    picker.handle_key(Key::Down); // main
    picker.handle_key(Key::Char(' '));
    picker.handle_key(Key::Down); // develop
    picker.handle_key(Key::Char(' '));
    picker.handle_key(Key::Up);
    picker.handle_key(Key::Char(' ')); // unselect main
    assert_eq!(
        picker.handle_key(Key::Enter),
        PickerOutcome::PickedMany(vec!["develop".into()])
    );
}

#[test]
fn show_all_clears_the_selection() {
    let mut picker = multi(&["main", "develop"]);
    assert_eq!(picker.cursor(), 0, "Show All row");
    assert_eq!(
        picker.handle_key(Key::Char(' ')),
        PickerOutcome::PickedMany(Vec::new())
    );
}

#[test]
fn a_typed_glob_can_be_added_as_a_pattern() {
    let mut picker = multi(&[]);
    type_text(&mut picker, "feature/*");
    assert_eq!(
        picker.rows()[..2],
        [Row::ShowAll, Row::Pattern("feature/*".into())]
    );
    assert_eq!(picker.visible_values(), ["feature/login", "feature/search"]);
    picker.handle_key(Key::Down);
    picker.handle_key(Key::Char(' '));
    assert_eq!(picker.selected(), ["feature/*"]);
    assert_eq!(
        picker.handle_key(Key::Enter),
        PickerOutcome::PickedMany(vec!["feature/*".into()])
    );
}

#[test]
fn selected_patterns_stay_listed_so_they_can_be_removed() {
    let mut picker = multi(&["feature/*", "main"]);
    assert_eq!(picker.rows()[1], Row::Pattern("feature/*".into()));
    picker.handle_key(Key::Down);
    picker.handle_key(Key::Char(' '));
    assert_eq!(picker.selected(), ["main"]);
}

// ---- rendering and mouse ----

#[test]
fn clicks_pick_in_single_mode_and_toggle_in_multi_mode() {
    let mut picker = single();
    let (_, hits) = render(&picker);
    let row = hits.row(1).unwrap();
    assert_eq!(
        picker.handle_click(&hits, row.x + 1, row.y),
        PickerOutcome::Picked("develop".into())
    );
    assert_eq!(
        picker.handle_click(&hits, 0, 0),
        PickerOutcome::Cancelled,
        "outside"
    );

    let mut picker = multi(&[]);
    let (_, hits) = render(&picker);
    let main = hits.row(1).unwrap();
    assert_eq!(
        picker.handle_click(&hits, main.x, main.y),
        PickerOutcome::Pending
    );
    assert_eq!(picker.selected(), ["main"]);
    let show_all = hits.row(0).unwrap();
    assert_eq!(
        picker.handle_click(&hits, show_all.x, show_all.y),
        PickerOutcome::PickedMany(Vec::new())
    );
    let query = hits.query();
    assert_eq!(hits.hit(query.x, query.y), PickerHit::Query);
}

#[test]
fn opens_at_an_anchor_or_centered() {
    let (_, hits) = render(&single());
    let area = hits.area();
    assert_eq!(area.x, (80 - area.width) / 2);
    let anchored = single().with_anchor(Position::new(3, 1));
    let (_, hits) = render(&anchored);
    assert_eq!((hits.area().x, hits.area().y), (3, 1));
}

#[test]
fn the_hit_map_reports_the_query_cursor() {
    let mut picker = single();
    type_text(&mut picker, "ma");
    let (_, hits) = render(&picker);
    let query = hits.query();
    assert_eq!(
        hits.cursor(),
        Some(Position::new(query.x + 4, query.y)),
        "after `> ma`"
    );
}
