//! Mouse and context-menu behavior, driven through real renders so clicks
//! resolve against the hit-maps of what was drawn.

use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use pretty_assertions::assert_eq;
use tgg_actions::MenuItem;
use tgg_testkit::FixtureRepo;
use tgg_ui::HitTarget;

use super::*;
use crate::testing::Harness;

const W: u16 = 110;
/// Tall enough for the whole commit menu (17 lines) without scrolling.
const H: u16 = 24;

fn mouse(kind: MouseEventKind, x: u16, y: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind,
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    })
}

fn click(h: &mut Harness, button: MouseButton, x: u16, y: u16) {
    let commands = h.state.update(mouse(MouseEventKind::Down(button), x, y));
    h.settle(commands);
    h.draw(W, H);
}

fn labels(state: &AppState) -> Vec<String> {
    state
        .menu
        .as_ref()
        .map(|m| {
            m.items()
                .iter()
                .filter_map(|item| match item {
                    MenuItem::Entry(e) => Some(e.label.clone()),
                    MenuItem::Separator => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Screen position of the first cell showing `text`.
fn find(h: &Harness, text: &str) -> (u16, u16) {
    h.find_on_screen(text)
        .unwrap_or_else(|| panic!("{text:?} not on screen"))
}

fn open(repo: &FixtureRepo) -> Harness {
    let mut h = Harness::open(repo);
    h.draw(W, H);
    h
}

#[test]
fn left_click_selects_the_row() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    let (_, y) = find(&h, "Update docs");
    click(&mut h, MouseButton::Left, 100, y);
    assert_eq!(h.selected_subject(), Some("Update docs"));
    click(&mut h, MouseButton::Left, 100, 0);
    assert_eq!(h.state.selected, 0, "the uncommitted row");
}

#[test]
fn clicking_a_badge_selects_its_commit() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    let (x, y) = find(&h, "develop");
    click(&mut h, MouseButton::Left, x + 1, y);
    assert_eq!(h.selected_subject(), Some("Return a value"));
}

#[test]
fn wheel_scrolls_the_view_and_loads_more_history() {
    let repo = FixtureRepo::sample();
    let mut h = Harness::with_page_size(&repo, 3);
    h.draw(W, 4);
    for _ in 0..4 {
        let commands = h.state.update(mouse(MouseEventKind::ScrollDown, 50, 1));
        h.settle(commands);
        h.draw(W, 4);
    }
    assert_eq!(
        h.state.commits.len(),
        7,
        "scrolling near the end loads older pages"
    );
    assert!(h.state.offset > 0);
    let offset = h.state.offset;
    h.state.update(mouse(MouseEventKind::ScrollUp, 50, 1));
    assert_eq!(h.state.offset, offset.saturating_sub(SCROLL_LINES));
}

#[test]
fn right_click_on_a_branch_badge_opens_the_branch_menu() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    let (x, y) = find(&h, "develop");
    click(&mut h, MouseButton::Right, x + 1, y);
    assert_eq!(
        h.selected_subject(),
        Some("Return a value"),
        "right-click selects the row too"
    );
    let labels = labels(&h.state);
    assert_eq!(
        labels.first().map(String::as_str),
        Some("Checkout Branch"),
        "{labels:?}"
    );
    assert!(labels.contains(&"Rename Branch...".to_owned()));
}

#[test]
fn right_click_on_the_current_branch_has_no_checkout() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    let (x, y) = find(&h, " main ");
    click(&mut h, MouseButton::Right, x + 2, y);
    let labels = labels(&h.state);
    assert!(
        !labels.contains(&"Checkout Branch".to_owned()),
        "{labels:?}"
    );
    assert!(labels.contains(&"Push Branch...".to_owned()), "{labels:?}");
}

#[test]
fn right_click_on_a_commit_opens_the_commit_menu() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    let (_, y) = find(&h, "Update docs");
    click(&mut h, MouseButton::Right, 100, y);
    let labels = labels(&h.state);
    assert_eq!(
        labels.first().map(String::as_str),
        Some("Add Tag..."),
        "{labels:?}"
    );
    assert!(labels.contains(&"Copy Commit Hash to Clipboard".to_owned()));
}

#[test]
fn menu_key_opens_the_menu_for_the_selected_row() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    h.key(KeyCode::Char('H'));
    h.key(KeyCode::Char('m'));
    assert_eq!(
        labels(&h.state).first().map(String::as_str),
        Some("Add Tag...")
    );
}

#[test]
fn escape_closes_the_menu_without_quitting() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    h.key(KeyCode::Char('j'));
    h.key(KeyCode::Char('m'));
    assert!(h.state.menu.is_some());
    h.key(KeyCode::Esc);
    assert!(h.state.menu.is_none());
    assert!(!h.state.should_quit);
}

#[test]
fn keys_go_to_the_open_menu() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    h.key(KeyCode::Char('j'));
    h.key(KeyCode::Char('m'));
    h.draw(W, H);
    let selected_before = h.state.selected;
    h.key(KeyCode::Char('j'));
    assert_eq!(
        h.state.selected, selected_before,
        "j moves inside the menu, not the graph"
    );
    h.key(KeyCode::Enter);
    assert!(h.state.menu.is_none(), "choosing closes the menu");
    let message = h.state.message.clone().unwrap_or_default();
    assert!(message.contains("Create Branch"), "{message}");
}

#[test]
fn clicking_outside_closes_the_menu() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    let (_, y) = find(&h, "Update docs");
    click(&mut h, MouseButton::Right, 100, y);
    assert!(h.state.menu.is_some());
    click(&mut h, MouseButton::Left, 0, H - 2);
    assert!(h.state.menu.is_none());
}

#[test]
fn clicking_a_menu_item_chooses_it() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    let (_, y) = find(&h, "Update docs");
    click(&mut h, MouseButton::Right, 10, y);
    let (x, item_y) = find(&h, "Copy Commit Hash to Clipboard");
    click(&mut h, MouseButton::Left, x, item_y);
    assert!(h.state.menu.is_none());
    assert!(
        h.state
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("Copy Commit Hash")
    );
}

#[test]
fn graph_hits_come_from_the_last_frame() {
    let repo = FixtureRepo::sample();
    let h = open(&repo);
    assert_eq!(h.state.graph_hits.hit(100, 1), Some(&HitTarget::Commit(0)));
}
