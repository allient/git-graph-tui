//! The inline commit details panel and the diff view, against real
//! fixture repositories.

use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use pretty_assertions::assert_eq;
use tgg_testkit::FixtureRepo;

use super::*;
use crate::testing::Harness;

/// Wide enough for full hashes beside the Source Control pane.
const W: u16 = 150;
const H: u16 = 30;

fn open(repo: &FixtureRepo) -> Harness {
    let mut h = Harness::open(repo);
    h.draw(W, H);
    h
}

fn press(h: &mut Harness, code: KeyCode) {
    h.key(code);
    h.draw(W, H);
}

fn click(h: &mut Harness, x: u16, y: u16) {
    let commands = h.state.update(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }));
    h.settle(commands);
    h.draw(W, H);
}

fn click_text(h: &mut Harness, text: &str) {
    let (x, y) = h
        .find_on_screen(text)
        .unwrap_or_else(|| panic!("{text:?} not on screen"));
    click(h, x, y);
}

/// Selects the commit with `subject` using the keyboard.
fn select(h: &mut Harness, subject: &str) {
    press(h, KeyCode::Char('g'));
    while h.selected_subject() != Some(subject) {
        let before = h.state.selected;
        press(h, KeyCode::Down);
        assert_ne!(h.state.selected, before, "{subject:?} not found");
    }
}

fn hash(repo: &FixtureRepo, subject: &str) -> String {
    repo.git(&["log", "--all", "--format=%H", &format!("--grep={subject}")])
}

#[test]
fn enter_opens_the_details_under_the_commit_and_esc_closes_them() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    select(&mut h, "Add lib");
    press(&mut h, KeyCode::Enter);

    let full = hash(&repo, "Add lib");
    assert!(h.find_on_screen(&full).is_some(), "commit hash");
    assert!(
        h.find_on_screen("Fixture Author <fixture@example.com>")
            .is_some()
    );
    assert!(h.find_on_screen("Changed files (1)").is_some());
    assert!(h.find_on_screen("lib.rs").is_some());
    // The panel opens right below the commit's row.
    let (_, row) = h.find_on_screen("Add lib").unwrap();
    let (_, panel) = h.find_on_screen("Commit:").unwrap();
    assert_eq!(panel, row + 1);

    press(&mut h, KeyCode::Esc);
    assert!(h.state.details.is_none());
    assert!(h.find_on_screen("Commit:").is_none());
    assert!(!h.state.should_quit, "Esc closes the panel, not the app");
}

#[test]
fn clicking_a_commit_toggles_its_details() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    click_text(&mut h, "Update docs");
    assert_eq!(h.selected_subject(), Some("Update docs"));
    assert!(h.find_on_screen("Changed files (1)").is_some());

    click_text(&mut h, "Add main");
    assert_eq!(h.selected_subject(), Some("Add main"));
    assert!(h.find_on_screen(&hash(&repo, "Add main")).is_some());
    assert!(h.find_on_screen(&hash(&repo, "Update docs")).is_none());

    click_text(&mut h, "Add main");
    assert!(h.state.details.is_none());
}

#[test]
fn opening_a_file_shows_its_diff_and_esc_returns_to_the_details() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    select(&mut h, "Add lib");
    press(&mut h, KeyCode::Enter);
    press(&mut h, KeyCode::Enter);

    assert!(h.state.diff.is_some());
    assert!(h.find_on_screen("+pub fn lib() {}").is_some());
    press(&mut h, KeyCode::Esc);
    assert!(h.state.diff.is_none());
    assert!(h.find_on_screen("Changed files (1)").is_some());
}

#[test]
fn clicking_a_file_opens_its_diff() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    select(&mut h, "Update docs");
    press(&mut h, KeyCode::Enter);
    click_text(&mut h, "README.md");
    assert!(h.find_on_screen("+Docs.").is_some());
}

#[test]
fn clicking_a_parent_jumps_to_it_and_shows_its_details() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    select(&mut h, "Add lib");
    press(&mut h, KeyCode::Enter);
    let parent = hash(&repo, "Add main");
    click_text(&mut h, &parent[..8]);

    assert_eq!(h.selected_subject(), Some("Add main"));
    assert!(h.find_on_screen(&parent).is_some());
}

#[test]
fn a_refresh_keeps_the_details_open() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    select(&mut h, "Add lib");
    press(&mut h, KeyCode::Enter);
    press(&mut h, KeyCode::Char('r'));
    assert!(h.find_on_screen("Changed files (1)").is_some());
    assert_eq!(h.selected_subject(), Some("Add lib"));
}

#[test]
fn enter_on_the_uncommitted_row_does_nothing() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    press(&mut h, KeyCode::Char('g'));
    press(&mut h, KeyCode::Enter);
    assert!(h.state.details.is_none());
}

#[test]
fn v_shows_the_selected_file_as_it_was_in_that_commit() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    select(&mut h, "Add lib");
    press(&mut h, KeyCode::Enter);
    assert!(
        h.find_on_screen("enter diff  v view file  t difftool")
            .is_some(),
        "key hint"
    );
    press(&mut h, KeyCode::Char('v'));

    let short = &hash(&repo, "Add lib")[..7];
    assert!(h.state.diff.is_some());
    assert!(h.find_on_screen(&format!("src/lib.rs @ {short}")).is_some());
    // The content then, not the later "Return a value" version, numbered from 1.
    assert!(h.find_on_screen("1 │ pub fn lib() {}").is_some());
    assert!(h.find_on_screen("pub fn lib() -> u8").is_none());
    assert_eq!(h.state.message, None);

    press(&mut h, KeyCode::Esc);
    assert!(h.state.diff.is_none());
    assert!(h.find_on_screen("Changed files (1)").is_some());
}

#[test]
fn v_on_a_file_the_commit_deleted_shows_the_git_error() {
    let repo = FixtureRepo::new();
    repo.commit_file("gone.txt", "bye\n", "Add gone");
    repo.git(&["rm", "--quiet", "--", "gone.txt"]);
    repo.git(&["commit", "--quiet", "--message", "Remove gone"]);
    let mut h = open(&repo);
    select(&mut h, "Remove gone");
    press(&mut h, KeyCode::Enter);
    press(&mut h, KeyCode::Char('v'));

    assert!(h.state.diff.is_none());
    let message = h.state.message.clone().unwrap_or_default();
    assert!(message.contains("gone.txt"), "{message}");
    assert!(message.contains("git show"), "{message}");
    // The details stay open.
    assert!(h.find_on_screen("Changed files (1)").is_some());
}
