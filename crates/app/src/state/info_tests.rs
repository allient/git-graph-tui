//! "View Details" on a tag badge: the read-only tag info dialog.

use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ggt_testkit::FixtureRepo;
use pretty_assertions::assert_eq;

use super::*;
use crate::testing::Harness;

const W: u16 = 110;
const H: u16 = 24;

fn click(h: &mut Harness, button: MouseButton, x: u16, y: u16) {
    let commands = h.state.update(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(button),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }));
    h.settle(commands);
    h.draw(W, H);
}

/// Right-clicks the badge `tag`, then picks "View Details".
fn view_details(h: &mut Harness, tag: &str) {
    let (x, y) = h
        .find_on_screen(tag)
        .unwrap_or_else(|| panic!("{tag:?} not on screen"));
    click(h, MouseButton::Right, x + 1, y);
    let (x, y) = h.find_on_screen("View Details").expect("tag menu");
    click(h, MouseButton::Left, x, y);
}

fn open(repo: &FixtureRepo) -> Harness {
    let mut h = Harness::open(repo);
    h.draw(W, H);
    h
}

#[test]
fn annotated_tags_show_the_tagger_and_message() {
    let repo = FixtureRepo::sample();
    repo.checkout("develop");
    repo.annotated_tag("v0.2.0", "Second release\n\nWith notes.");
    let mut h = open(&repo);
    view_details(&mut h, "v0.2.0");

    let info = h.state.info.as_ref().expect("info dialog");
    assert_eq!(info.title, "Tag v0.2.0");
    let commit = repo.git(&["rev-parse", "v0.2.0^{commit}"]);
    let tag_time = repo.git(&[
        "for-each-ref",
        "--format=%(taggerdate:unix)",
        "refs/tags/v0.2.0",
    ]);
    let date = ggt_ui::format_date(tag_time.parse().unwrap());
    assert_eq!(
        info.lines,
        [
            format!("Commit:  {commit}"),
            "Tagger:  Fixture Author <fixture@example.com>".to_owned(),
            format!("Date:    {date}"),
            String::new(),
            "Second release".to_owned(),
            String::new(),
            "With notes.".to_owned(),
        ]
    );
    assert!(h.find_on_screen("Second release").is_some(), "drawn");
    h.key(KeyCode::Esc);
    assert!(h.state.info.is_none());
}

#[test]
fn lightweight_tags_say_so_and_any_click_closes() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    view_details(&mut h, "v0.1.0");
    let info = h.state.info.as_ref().expect("info dialog");
    assert_eq!(
        info.lines.last().map(String::as_str),
        Some("Lightweight tag: no tagger or message.")
    );
    click(&mut h, MouseButton::Left, 0, 0);
    assert!(h.state.info.is_none());
}

#[test]
fn enter_closes_and_other_keys_are_ignored() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    view_details(&mut h, "v0.1.0");
    h.key(KeyCode::Char('j'));
    assert!(h.state.info.is_some(), "keys don't reach the graph");
    h.key(KeyCode::Enter);
    assert!(h.state.info.is_none());
}
