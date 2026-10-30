//! Diffs and "Open File" from the Source Control pane, and side by side in
//! the diff view, against real fixture repositories.

use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ggt_git::show::LineKind;
use ggt_testkit::FixtureRepo;
use pretty_assertions::assert_eq;

use super::*;
use crate::external;
use crate::testing::Harness;

const W: u16 = 110;
const WIDE: u16 = 200;
const H: u16 = 30;

fn open(repo: &FixtureRepo) -> Harness {
    let mut h = Harness::open(repo);
    h.draw(W, H);
    h
}

fn press(h: &mut Harness, code: KeyCode) {
    h.key(code);
    h.draw(h.screen.area.width, H);
}

fn click(h: &mut Harness, x: u16, y: u16) {
    let commands = h.state.update(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }));
    h.settle(commands);
    h.draw(h.screen.area.width, H);
}

fn find(h: &Harness, text: &str) -> (u16, u16) {
    h.find_on_screen(text)
        .unwrap_or_else(|| panic!("{text:?} not on screen"))
}

/// A button on the pane line showing `text`.
fn button_on(h: &Harness, text: &str, button: &str) -> (u16, u16) {
    let (_, y) = find(h, text);
    let pane = h.state.source_control.hits.clone();
    let x = (0..h.screen.area.width)
        .take_while(|&x| pane.contains(x, y))
        .filter(|&x| h.screen[(x, y)].symbol() == button)
        .last()
        .unwrap_or_else(|| panic!("no {button:?} on the {text:?} line"));
    (x, y)
}

/// Focuses the pane and moves down `rows` from the message box (the
/// Commit button is the first stop, the first list item the second).
fn select_in_pane(h: &mut Harness, rows: usize) {
    press(h, KeyCode::Char('c'));
    for _ in 0..rows {
        press(h, KeyCode::Down);
    }
}

/// The added and deleted lines of the open diff.
fn changed_lines(h: &Harness) -> Vec<String> {
    let diff = h.state.diff.as_ref().expect("a diff is open");
    diff.view
        .diff()
        .lines
        .iter()
        .filter(|l| matches!(l.kind, LineKind::Added | LineKind::Deleted))
        .map(|l| l.text.clone())
        .collect()
}

#[test]
fn enter_on_a_changed_file_shows_its_working_tree_diff() {
    let repo = FixtureRepo::sample();
    repo.write_file("README.md", "# sample\n\nNew docs.\n");
    let mut h = open(&repo);
    // Message → Commit → Changes header → README.md.
    select_in_pane(&mut h, 3);
    press(&mut h, KeyCode::Enter);
    assert_eq!(changed_lines(&h), ["-Docs.", "+New docs."]);
    assert!(
        h.find_on_screen("README.md (working tree) (+1 -1)")
            .is_some()
    );
    assert!(
        h.find_on_screen("diff · s side by side · esc closes")
            .is_some()
    );
    press(&mut h, KeyCode::Esc);
    assert!(h.state.diff.is_none());
    // Space still stages.
    press(&mut h, KeyCode::Char(' '));
    let status = repo.git(&["status", "--porcelain"]);
    assert!(status.starts_with("M  README.md"), "{status}");
}

#[test]
fn enter_on_a_staged_file_shows_the_staged_diff() {
    let repo = FixtureRepo::sample();
    repo.write_file("README.md", "# sample\n\nStaged.\n");
    repo.git(&["add", "README.md"]);
    repo.write_file("README.md", "# sample\n\nLater.\n");
    let mut h = open(&repo);
    // Message → Commit → Staged Changes header → README.md.
    select_in_pane(&mut h, 3);
    press(&mut h, KeyCode::Enter);
    assert_eq!(changed_lines(&h), ["-Docs.", "+Staged."]);
    assert!(h.find_on_screen("README.md (staged)").is_some());
}

#[test]
fn an_untracked_file_shows_every_line_added() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    let (x, y) = find(&h, "notes.txt");
    click(&mut h, x + 1, y);
    assert_eq!(changed_lines(&h), ["+uncommitted"]);
    assert!(
        h.find_on_screen("notes.txt (working tree) (+1 -0)")
            .is_some()
    );
    assert_eq!(h.state.message, None, "exit code 1 is not an error here");
}

#[test]
fn open_changes_shows_every_file_of_the_section_in_one_view() {
    let repo = FixtureRepo::sample();
    repo.write_file("README.md", "# sample\n\nNew docs.\n");
    let mut h = open(&repo);
    let (x, y) = button_on(&h, "▾ Changes", "⧉");
    click(&mut h, x, y);
    assert!(h.find_on_screen("Changes (working tree) (+2 -1)").is_some());
    let (_, readme) = find(&h, "▾ README.md M");
    let (_, notes) = find(&h, "▾ notes.txt U");
    assert!(readme < notes, "in the pane's order");
    assert_eq!(changed_lines(&h), ["-Docs.", "+New docs.", "+uncommitted"]);
}

#[test]
fn o_opens_the_staged_section() {
    let repo = FixtureRepo::sample();
    repo.write_file("README.md", "# sample\n\nStaged.\n");
    repo.git(&["add", "README.md", "notes.txt"]);
    let mut h = open(&repo);
    // Message → Commit → Staged Changes header.
    select_in_pane(&mut h, 2);
    press(&mut h, KeyCode::Char('o'));
    assert!(h.find_on_screen("Staged Changes (staged)").is_some());
    assert!(h.find_on_screen("▾ README.md M").is_some());
    assert!(h.find_on_screen("▾ notes.txt A").is_some());
    assert_eq!(changed_lines(&h), ["-Docs.", "+Staged.", "+uncommitted"]);
}

#[test]
fn a_pager_shows_a_pane_diff_instead() {
    let repo = FixtureRepo::sample();
    repo.write_file("README.md", "changed\n");
    let mut h = open(&repo);
    h.state.pager = Some("delta".into());
    select_in_pane(&mut h, 3);
    press(&mut h, KeyCode::Enter);
    assert!(h.state.diff.is_none());
    let repo_path = h.state.repo.to_string_lossy().into_owned();
    assert_eq!(
        h.external,
        [external::pager_command(
            &h.state.repo,
            &show::working_diff_args(false, &["README.md".to_owned()]),
            "delta"
        )]
    );
    assert!(h.external[0].args[1].contains(&repo_path));
}

#[test]
fn e_and_the_open_file_button_hand_the_file_to_the_editor() {
    let repo = FixtureRepo::sample();
    repo.write_file("README.md", "changed\n");
    let mut h = open(&repo);
    h.state.editor = "nano -w".into();
    select_in_pane(&mut h, 3);
    press(&mut h, KeyCode::Char('e'));
    let expected = external::editor_command(&h.state.repo, "nano -w", "README.md");
    assert!(expected.refresh, "the repository is reloaded afterwards");
    assert_eq!(h.external, [expected]);

    let (x, y) = button_on(&h, "notes.txt", "↗");
    click(&mut h, x, y);
    assert_eq!(
        h.external.last(),
        Some(&external::editor_command(
            &h.state.repo,
            "nano -w",
            "notes.txt"
        ))
    );
}

#[test]
fn a_deleted_file_cannot_be_opened() {
    let repo = FixtureRepo::sample();
    std::fs::remove_file(repo.path().join("README.md")).unwrap();
    let mut h = open(&repo);
    select_in_pane(&mut h, 3);
    press(&mut h, KeyCode::Char('e'));
    assert!(h.external.is_empty());
    assert_eq!(
        h.state.message.as_deref(),
        Some("Can't open README.md: it was deleted.")
    );
}

#[test]
fn s_shows_diffs_side_by_side_for_the_rest_of_the_session() {
    let repo = FixtureRepo::sample();
    repo.write_file("README.md", "# sample\n\nNew docs.\n");
    let mut h = Harness::open(&repo);
    h.draw(WIDE, H);
    select_in_pane(&mut h, 3);
    press(&mut h, KeyCode::Enter);
    press(&mut h, KeyCode::Char('s'));
    assert!(h.state.side_by_side);
    // Old on the left, new on the right, on one row.
    let (old_x, row) = find(&h, "Docs.");
    let (new_x, new_row) = find(&h, "New docs.");
    assert_eq!(row, new_row);
    assert!(new_x > old_x + 40, "{old_x} {new_x}");
    // The next diff opens side by side too.
    press(&mut h, KeyCode::Esc);
    press(&mut h, KeyCode::Down);
    press(&mut h, KeyCode::Enter);
    assert!(h.state.diff.as_ref().unwrap().view.side_by_side());
    press(&mut h, KeyCode::Char('s'));
    assert!(!h.state.side_by_side);
}

#[test]
fn side_by_side_needs_a_wide_diff() {
    let repo = FixtureRepo::sample();
    repo.write_file("README.md", "changed\n");
    let mut h = open(&repo);
    select_in_pane(&mut h, 3);
    press(&mut h, KeyCode::Enter);
    press(&mut h, KeyCode::Char('s'));
    assert!(!h.state.side_by_side);
    assert_eq!(
        h.state.message.as_deref(),
        Some("Widen the diff to at least 100 columns to show it side by side.")
    );
}
