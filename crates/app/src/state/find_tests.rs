//! Find (`/`): search the loaded commits and step through the matches.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;
use tgg_testkit::FixtureRepo;
use tgg_ui::Theme;

use crate::state::{Command, Event};
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
    // "docs" alone also finds the fixture's stash, "On main: WIP on docs".
    find(&mut h, "update docs");
    assert_eq!(h.selected_subject(), Some("Update docs"));
    let status = status_line(&h);
    assert!(status.contains("Find: update docs"), "{status:?}");
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

    // Seven commits and the fixture's stash row.
    find(&mut h, "FIXTURE AUTHOR");
    assert!(status_line(&h).contains("1 of 8"), "{:?}", status_line(&h));
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
    find(&mut h, "update docsx");
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

/// 30 commits on `main`, `Commit 1` (the root) to `Commit 30`, read three at
/// a time.
fn paged() -> (FixtureRepo, Harness) {
    let repo = FixtureRepo::new();
    for i in 1..=30 {
        repo.commit_file("log.txt", &format!("{i}\n"), &format!("Commit {i}"));
    }
    let mut h = Harness::with_page_size(&repo, 3);
    h.draw(W, H);
    (repo, h)
}

/// Sends a key without waiting for the git reads it starts.
fn press_unsettled(h: &mut Harness, code: KeyCode) -> Vec<Command> {
    let commands = h
        .state
        .update(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
    h.draw(W, H);
    commands
}

#[test]
fn a_query_with_no_loaded_match_searches_older_history() {
    let (_repo, mut h) = paged();
    assert!(!h.state.commits.iter().any(|c| c.subject == "Commit 5"));
    find(&mut h, "Commit ");
    let commands = press_unsettled(&mut h, KeyCode::Char('5'));
    assert!(
        status_line(&h).contains("Searching older history…"),
        "{:?}",
        status_line(&h)
    );
    h.settle(commands);
    h.draw(W, H);
    assert_eq!(h.selected_subject(), Some("Commit 5"));
    assert!(status_line(&h).contains("1 of 1"), "{:?}", status_line(&h));
}

#[test]
fn no_matches_is_said_only_once_history_is_complete() {
    let (_repo, mut h) = paged();
    find(&mut h, "Commit ");
    let commands = press_unsettled(&mut h, KeyCode::Char('x'));
    let status = status_line(&h);
    assert!(status.contains("Searching older history…"), "{status:?}");
    assert!(!status.contains("No matches"), "{status:?}");
    h.settle(commands);
    h.draw(W, H);
    assert!(h.state.history_complete);
    assert!(
        status_line(&h).contains("No matches"),
        "{:?}",
        status_line(&h)
    );
}

#[test]
fn typing_a_new_query_restarts_the_search() {
    let (_repo, mut h) = paged();
    find(&mut h, "Commit ");
    let mut commands = press_unsettled(&mut h, KeyCode::Char('9'));
    commands.extend(press_unsettled(&mut h, KeyCode::Backspace));
    commands.extend(press_unsettled(&mut h, KeyCode::Char('4')));
    h.settle(commands);
    assert_eq!(h.selected_subject(), Some("Commit 4"));
}

#[test]
fn stepping_past_the_last_loaded_match_loads_more_before_wrapping() {
    let (_repo, mut h) = paged();
    // Commit 29, 28, ..., 20 and Commit 2: eleven matches, most not loaded.
    find(&mut h, "Commit 2");
    assert_eq!(h.selected_subject(), Some("Commit 29"));
    let mut seen = Vec::new();
    for _ in 0..11 {
        press(&mut h, KeyCode::Enter);
        seen.push(h.selected_subject().unwrap_or_default().to_owned());
    }
    let expected: Vec<String> = (20..=28)
        .rev()
        .map(|i| format!("Commit {i}"))
        .chain(["Commit 2".to_owned(), "Commit 29".to_owned()])
        .collect();
    assert_eq!(seen, expected);
    assert!(status_line(&h).contains("1 of 11"), "{:?}", status_line(&h));
}
