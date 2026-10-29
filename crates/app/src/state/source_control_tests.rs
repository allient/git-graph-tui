//! The Source Control pane beside the graph, against real fixture repositories.

use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use pretty_assertions::assert_eq;
use tgg_testkit::FixtureRepo;

use super::*;
use crate::testing::Harness;

const W: u16 = 110;
const H: u16 = 24;

fn open(repo: &FixtureRepo) -> Harness {
    let mut h = Harness::open(repo);
    h.draw(W, H);
    h
}

fn press(h: &mut Harness, code: KeyCode) {
    h.key(code);
    h.draw(W, H);
}

fn type_text(h: &mut Harness, text: &str) {
    for ch in text.chars() {
        press(h, KeyCode::Char(ch));
    }
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

fn find(h: &Harness, text: &str) -> (u16, u16) {
    h.find_on_screen(text)
        .unwrap_or_else(|| panic!("{text:?} not on screen"))
}

/// The `+`/`−` button at the end of the pane line showing `text`.
fn button_of(h: &Harness, text: &str) -> (u16, u16) {
    let (_, y) = find(h, text);
    let pane = h.state.source_control.hits.clone();
    let right = (0..W).take_while(|&x| pane.contains(x, y)).last().unwrap();
    (right, y)
}

fn status(repo: &FixtureRepo) -> String {
    repo.git(&["status", "--porcelain", "--untracked-files=all"])
}

#[test]
fn source_control_and_the_graph_sit_side_by_side() {
    let repo = FixtureRepo::sample();
    let h = open(&repo);
    let (pane_x, pane_y) = find(&h, "SOURCE CONTROL");
    let (graph_x, graph_y) = find(&h, "Graph");
    assert_eq!((pane_x, pane_y), (1, 0));
    assert_eq!(
        graph_y, 1,
        "the toolbar is above the graph, the pane beside both"
    );
    assert!(graph_x > 30, "{graph_x}");
    // The sample repo's untracked notes.txt is listed under Changes.
    let (_, changes) = find(&h, "▾ Changes");
    let (x, notes) = find(&h, "notes.txt");
    assert!(x < graph_x && notes == changes + 1);
}

#[test]
fn stage_a_file_type_a_message_and_commit() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    let (x, y) = button_of(&h, "notes.txt");
    click(&mut h, x, y);
    assert_eq!(status(&repo), "A  notes.txt");
    assert!(h.find_on_screen("▾ Staged Changes").is_some());

    assert!(
        h.state.source_control.focus,
        "a click in the pane takes the keyboard"
    );
    let (x, y) = find(&h, " Message");
    click(&mut h, x + 2, y);
    type_text(&mut h, "Add notes");
    press(&mut h, KeyCode::Enter);

    assert_eq!(repo.git(&["log", "-1", "--format=%s"]), "Add notes");
    assert_eq!(status(&repo), "");
    assert_eq!(
        h.state.source_control.view.message(),
        "",
        "cleared after the commit"
    );
    assert!(h.find_on_screen("notes.txt").is_none());
    let message = h.state.message.clone().unwrap_or_default();
    assert!(message.contains("✓ Commit"), "{message}");
}

#[test]
fn keys_stage_and_unstage_files() {
    let repo = FixtureRepo::sample();
    repo.write_file("README.md", "changed\n");
    let mut h = open(&repo);
    press(&mut h, KeyCode::Char('c'));
    // Message → Commit button → Changes header → README.md.
    for _ in 0..3 {
        press(&mut h, KeyCode::Down);
    }
    press(&mut h, KeyCode::Char(' '));
    assert!(
        status(&repo).starts_with("M  README.md"),
        "{}",
        status(&repo)
    );
    // README.md moved to Staged Changes, the first file of the list.
    press(&mut h, KeyCode::Home);
    press(&mut h, KeyCode::Down);
    press(&mut h, KeyCode::Char('-'));
    assert!(
        status(&repo).starts_with(" M README.md"),
        "{}",
        status(&repo)
    );
}

#[test]
fn committing_needs_a_message_and_staged_files() {
    let repo = FixtureRepo::sample();
    let before = repo.git(&["rev-parse", "HEAD"]);
    let mut h = open(&repo);
    press(&mut h, KeyCode::Char('c'));
    press(&mut h, KeyCode::Enter);
    assert_eq!(
        h.state.message.as_deref(),
        Some("Type a commit message first.")
    );
    type_text(&mut h, "Nothing");
    press(&mut h, KeyCode::Enter);
    assert_eq!(
        h.state.message.as_deref(),
        Some("Nothing staged: stage files with + first.")
    );
    assert_eq!(repo.git(&["rev-parse", "HEAD"]), before);
    assert_eq!(
        h.state.source_control.view.message(),
        "Nothing",
        "kept for later"
    );
}

#[test]
fn escape_leaves_the_pane_without_quitting() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    press(&mut h, KeyCode::Char('c'));
    type_text(&mut h, "q");
    assert!(!h.state.should_quit, "q is typed into the message");
    press(&mut h, KeyCode::Esc);
    assert!(!h.state.source_control.focus);
    assert!(!h.state.should_quit);
    press(&mut h, KeyCode::Char('j'));
    assert_eq!(h.state.selected, 1, "keys go to the graph again");
}

#[test]
fn clicking_the_graph_takes_the_keyboard_back() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    let (x, y) = find(&h, " Message");
    click(&mut h, x + 2, y);
    assert!(h.state.source_control.focus);
    let (_, row) = find(&h, "Update docs");
    click(&mut h, 100, row);
    assert!(!h.state.source_control.focus);
    assert_eq!(h.selected_subject(), Some("Update docs"));
}

#[test]
fn open_source_control_from_the_uncommitted_menu_focuses_the_message() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    press(&mut h, KeyCode::Char('m'));
    let (x, y) = find(&h, "Open Source Control");
    click(&mut h, x, y);
    assert!(h.state.source_control.focus);
}

#[test]
fn a_narrow_terminal_hides_the_pane() {
    let repo = FixtureRepo::sample();
    let mut h = Harness::open(&repo);
    h.draw(80, H);
    assert!(h.find_on_screen("SOURCE CONTROL").is_none());
    h.key(KeyCode::Char('c'));
    assert!(!h.state.source_control.focus);
    assert_eq!(
        h.state.message.as_deref(),
        Some("Widen the terminal to at least 90 columns to show Source Control.")
    );
}
