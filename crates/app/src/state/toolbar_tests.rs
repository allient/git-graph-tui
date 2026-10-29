//! The toolbar, the branch filter and the repository menu's pickers,
//! against real fixture repositories.

use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use pretty_assertions::assert_eq;
use tgg_testkit::FixtureRepo;

use super::*;
use crate::testing::Harness;

const W: u16 = 110;
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

fn type_text(h: &mut Harness, text: &str) {
    for ch in text.chars() {
        press(h, KeyCode::Char(ch));
    }
}

fn mouse(h: &mut Harness, kind: MouseEventKind, x: u16, y: u16) {
    let commands = h.state.update(Event::Mouse(MouseEvent {
        kind,
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

fn click_text(h: &mut Harness, text: &str) {
    let (x, y) = find(h, text);
    mouse(h, MouseEventKind::Down(MouseButton::Left), x, y);
}

/// The first `text` on a line below `min_y`.
fn find_below(h: &Harness, text: &str, min_y: u16) -> (u16, u16) {
    let area = h.screen.area;
    (min_y + 1..area.bottom())
        .find_map(|y| {
            let line: String = (area.left()..area.right())
                .map(|x| h.screen[(x, y)].symbol())
                .collect();
            let byte = line.find(text)?;
            Some((u16::try_from(line[..byte].chars().count()).ok()?, y))
        })
        .unwrap_or_else(|| panic!("{text:?} not below line {min_y}"))
}

fn subjects(h: &Harness) -> Vec<&str> {
    h.state.commits.iter().map(|c| c.subject.as_str()).collect()
}

fn current_branch(repo: &FixtureRepo) -> String {
    repo.git(&["symbolic-ref", "--short", "HEAD"])
}

#[test]
fn the_toolbar_is_the_first_line() {
    let repo = FixtureRepo::sample();
    let h = open(&repo);
    let (_, y) = find(&h, "Branches: [Show All");
    assert_eq!(y, 0);
    assert!(h.find_on_screen("[⇣]").is_some());
}

#[test]
fn the_branch_filter_limits_the_graph_to_the_picked_branches() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    click_text(&mut h, "Show All");
    assert!(h.state.picker.is_some());
    type_text(&mut h, "feature/login");
    // Off the "Show All" row, onto the matching branch.
    press(&mut h, KeyCode::Down);
    press(&mut h, KeyCode::Char(' '));
    press(&mut h, KeyCode::Enter);

    assert_eq!(h.state.toolbar.branch_filter, vec!["feature/login"]);
    assert!(h.find_on_screen("Branches: [feature/login").is_some());
    let shown = subjects(&h);
    assert_eq!(shown[0], "Add login");
    assert!(!shown.contains(&"Update docs"), "{shown:?}");

    // Show All again.
    click_text(&mut h, "feature/login ▾");
    press(&mut h, KeyCode::PageUp); // the "Show All" row
    press(&mut h, KeyCode::Enter);
    assert!(h.state.toolbar.branch_filter.is_empty());
    assert!(subjects(&h).contains(&"Update docs"));
}

#[test]
fn the_branch_menu_adds_and_removes_a_branch_from_the_filter() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    let (x, y) = find(&h, "develop");
    mouse(&mut h, MouseEventKind::Down(MouseButton::Right), x + 1, y);
    click_text(&mut h, "Select in Branches Dropdown");
    assert_eq!(h.state.toolbar.branch_filter, vec!["develop"]);
    assert!(!subjects(&h).contains(&"Add login"));

    let (x, y) = find(&h, "develop  Return");
    mouse(&mut h, MouseEventKind::Down(MouseButton::Right), x + 1, y);
    click_text(&mut h, "Unselect in Branches Dropdown");
    assert!(h.state.toolbar.branch_filter.is_empty());
    assert!(subjects(&h).contains(&"Add login"));
}

#[test]
fn hiding_remote_branches_hides_their_badges_and_history() {
    let repo = FixtureRepo::sample();
    repo.checkout("feature/login");
    repo.push("feature/login");
    repo.checkout("main");
    repo.git(&["branch", "--delete", "--force", "feature/login"]);
    let mut h = open(&repo);
    assert!(subjects(&h).contains(&"Add login"));
    assert!(h.find_on_screen("origin/feature/login").is_some());

    click_text(&mut h, "Show Remote Branches");
    assert!(!h.state.toolbar.show_remotes);
    assert!(h.find_on_screen("[ ] Show Remote Branches").is_some());
    assert!(!subjects(&h).contains(&"Add login"));
    assert!(h.find_on_screen("origin").is_none(), "no remote badges");
}

#[test]
fn checkout_to_picks_any_branch_and_switches_to_it() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    click_text(&mut h, "[...]");
    click_text(&mut h, "Checkout to...");
    assert!(h.state.picker.is_some());
    type_text(&mut h, "develop");
    press(&mut h, KeyCode::Enter);
    // A local branch checkout runs without a dialog.
    assert_eq!(current_branch(&repo), "develop");
    assert!(
        h.state.message.as_deref().unwrap_or_default().contains('✓'),
        "{:?}",
        h.state.message
    );
}

#[test]
fn delete_branch_from_the_repository_menu_picks_the_branch() {
    let repo = FixtureRepo::sample();
    repo.git(&["branch", "old", "main"]);
    let mut h = open(&repo);
    click_text(&mut h, "[...]");
    // The "Branch" submenu row, below "Checkout to..." (the toolbar also says "Branches").
    let (_, below) = find(&h, "Checkout to...");
    let (x, y) = find_below(&h, "Branch", below);
    mouse(&mut h, MouseEventKind::Down(MouseButton::Left), x, y);
    click_text(&mut h, "Delete Branch...");
    type_text(&mut h, "old");
    press(&mut h, KeyCode::Enter);
    if matches!(h.state.dialog, Some(Dialog::Input { .. })) {
        press(&mut h, KeyCode::Enter); // keep "force" off
    }
    // Destructive: focus starts on Cancel, so move to Confirm first.
    assert!(matches!(h.state.dialog, Some(Dialog::Confirm { .. })));
    press(&mut h, KeyCode::Left);
    press(&mut h, KeyCode::Enter);
    let branches = repo.git(&["branch", "--format=%(refname:short)"]);
    assert!(!branches.lines().any(|b| b == "old"), "{branches}");
}

#[test]
fn escape_closes_the_picker_without_changes() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    click_text(&mut h, "[...]");
    click_text(&mut h, "Checkout to...");
    press(&mut h, KeyCode::Esc);
    assert!(h.state.picker.is_none());
    assert!(!h.state.should_quit);
    assert_eq!(current_branch(&repo), "main");
}

#[test]
fn tab_moves_to_the_toolbar_and_enter_presses_a_button() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    press(&mut h, KeyCode::Tab);
    assert!(h.state.toolbar.focus.is_some());
    // Branch filter → remotes → refresh → fetch.
    for _ in 0..3 {
        press(&mut h, KeyCode::Right);
    }
    press(&mut h, KeyCode::Enter);
    let message = h.state.message.clone().unwrap_or_default();
    assert!(message.contains("✓ Fetch"), "{message}");

    press(&mut h, KeyCode::Esc);
    assert!(h.state.toolbar.focus.is_none());
    assert!(!h.state.should_quit, "Esc leaves the toolbar first");
}

#[test]
fn a_filter_that_matches_no_branch_shows_no_commits() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    h.state.toolbar.branch_filter = vec!["nope/*".to_owned()];
    press(&mut h, KeyCode::Char('r'));
    assert!(h.state.commits.is_empty());
    assert!(h.state.history_complete);
}
