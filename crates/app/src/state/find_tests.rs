//! Find (`/`): search the loaded commits and step through the matches.

use crossterm::event::KeyCode;
use pretty_assertions::assert_eq;
use tgg_testkit::FixtureRepo;
use tgg_ui::Theme;

use crate::testing::Harness;

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

fn find(h: &mut Harness, query: &str) {
    press(h, KeyCode::Char('/'));
    for ch in query.chars() {
        press(h, KeyCode::Char(ch));
    }
}

fn status_line(h: &Harness) -> String {
    let area = h.screen.area;
    let y = area.bottom() - 1;
    (area.left()..area.right())
        .map(|x| h.screen[(x, y)].symbol())
        .collect()
}

#[test]
fn typing_jumps_to_the_first_matching_commit() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    find(&mut h, "docs");
    assert_eq!(h.selected_subject(), Some("Update docs"));
    let status = status_line(&h);
    assert!(status.contains("Find: docs"), "{status:?}");
    assert!(status.contains("1 of 1"), "{status:?}");
}

#[test]
fn enter_and_arrows_step_through_matches_and_wrap() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    // "Add login", "Add lib" and "Add main", newest first.
    find(&mut h, "add ");
    assert_eq!(h.selected_subject(), Some("Add login"));
    press(&mut h, KeyCode::Enter);
    assert_eq!(h.selected_subject(), Some("Add lib"));
    press(&mut h, KeyCode::Down);
    assert_eq!(h.selected_subject(), Some("Add main"));
    assert!(status_line(&h).contains("3 of 3"));
    press(&mut h, KeyCode::Enter);
    assert_eq!(
        h.selected_subject(),
        Some("Add login"),
        "wraps to the first"
    );
    press(&mut h, KeyCode::Up);
    assert_eq!(
        h.selected_subject(),
        Some("Add main"),
        "and back to the last"
    );
}

#[test]
fn finds_by_hash_author_date_and_ref_name() {
    let repo = FixtureRepo::sample();
    let hash = repo.git(&["log", "-1", "--format=%h", "--grep=Add lib"]);
    let mut h = open(&repo);
    find(&mut h, &hash);
    assert_eq!(h.selected_subject(), Some("Add lib"));
    press(&mut h, KeyCode::Esc);

    find(&mut h, "develop");
    assert_eq!(
        h.selected_subject(),
        Some("Return a value"),
        "the develop badge"
    );
    press(&mut h, KeyCode::Esc);

    find(&mut h, "v0.1");
    assert_eq!(
        h.selected_subject(),
        Some("Merge branch 'develop'"),
        "a tag"
    );
    press(&mut h, KeyCode::Esc);

    find(&mut h, "00:04");
    assert_eq!(h.selected_subject(), Some("Update docs"), "the date column");
    press(&mut h, KeyCode::Esc);

    find(&mut h, "FIXTURE AUTHOR");
    assert!(status_line(&h).contains("1 of 7"), "{:?}", status_line(&h));
}

#[test]
fn no_match_keeps_the_selection_and_says_so() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    press(&mut h, KeyCode::Char('j'));
    let before = h.state.selected;
    find(&mut h, "nothing like this");
    assert_eq!(h.state.selected, before);
    assert!(
        status_line(&h).contains("No matches"),
        "{:?}",
        status_line(&h)
    );
}

#[test]
fn escape_closes_find_and_keys_go_back_to_the_graph() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    find(&mut h, "q");
    assert!(!h.state.should_quit, "q is typed into the search");
    press(&mut h, KeyCode::Esc);
    assert!(h.state.find.is_none());
    assert!(!h.state.should_quit);
    let selected = h.state.selected;
    press(&mut h, KeyCode::Char('j'));
    assert_eq!(h.state.selected, selected + 1);
}

#[test]
fn backspace_edits_the_query() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    find(&mut h, "docsx");
    assert!(status_line(&h).contains("No matches"));
    press(&mut h, KeyCode::Backspace);
    assert_eq!(h.selected_subject(), Some("Update docs"));
}

#[test]
fn matching_rows_are_highlighted() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    find(&mut h, "docs");
    let (x, y) = h.find_on_screen("Update docs").unwrap();
    assert_eq!(h.screen[(x, y)].fg, Theme::default().tag);
    let (x, y) = h.find_on_screen("Add lib").unwrap();
    assert_ne!(h.screen[(x, y)].fg, Theme::default().tag);
}
