//! Running actions end to end: menu → input dialog → confirmation → git →
//! refresh, against real fixture repositories.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use pretty_assertions::assert_eq;
use tgg_model::HeadState;
use tgg_testkit::FixtureRepo;

use super::*;
use crate::testing::Harness;

const W: u16 = 110;
const H: u16 = 24;

fn mouse_down(button: MouseButton, x: u16, y: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(button),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    })
}

fn open(repo: &FixtureRepo) -> Harness {
    let mut h = Harness::open(repo);
    h.draw(W, H);
    h
}

fn click(h: &mut Harness, button: MouseButton, x: u16, y: u16) {
    let commands = h.state.update(mouse_down(button, x, y));
    h.settle(commands);
    h.draw(W, H);
}

/// Right-clicks the first place `on` appears, then clicks the menu entry `entry`.
fn choose(h: &mut Harness, on: &str, entry: &str) {
    let (x, y) = h
        .find_on_screen(on)
        .unwrap_or_else(|| panic!("{on:?} not on screen"));
    click(h, MouseButton::Right, x + 1, y);
    let (x, y) = h
        .find_on_screen(entry)
        .unwrap_or_else(|| panic!("menu entry {entry:?} not on screen"));
    click(h, MouseButton::Left, x, y);
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

fn current_branch(repo: &FixtureRepo) -> String {
    repo.git(&["symbolic-ref", "--short", "HEAD"])
}

#[test]
fn checkout_branch_from_its_badge_switches_head() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    choose(&mut h, "develop", "Checkout Branch");
    assert_eq!(current_branch(&repo), "develop");
    assert_eq!(
        h.state.status.head,
        HeadState::Branch("develop".into()),
        "refreshed after running"
    );
    assert!(
        h.state
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("Checkout Branch"),
        "{:?}",
        h.state.message
    );
}

#[test]
fn create_branch_asks_for_a_name_then_creates_it() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    let update_docs = repo.git(&["log", "--all", "--format=%H", "--grep=Update docs"]);
    choose(&mut h, "Update docs", "Create Branch...");
    assert!(
        matches!(h.state.dialog, Some(Dialog::Input { .. })),
        "asks for the branch name"
    );
    type_text(&mut h, "topic");
    press(&mut h, KeyCode::Enter);
    assert!(h.state.dialog.is_none());
    assert_eq!(repo.git(&["rev-parse", "topic"]), update_docs);
    assert!(
        h.state.refs.iter().any(|r| r.short_name() == "topic"),
        "refs reloaded"
    );
    assert_eq!(
        current_branch(&repo),
        "main",
        "checkout of the new branch is off by default"
    );
}

#[test]
fn invalid_branch_names_keep_the_dialog_open() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    choose(&mut h, "Update docs", "Create Branch...");
    type_text(&mut h, "bad name");
    press(&mut h, KeyCode::Enter);
    assert!(matches!(h.state.dialog, Some(Dialog::Input { .. })));
    assert!(repo.git(&["branch", "--list", "bad*"]).is_empty());
}

#[test]
fn cancelling_a_confirmation_changes_nothing() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    let head = repo.git(&["rev-parse", "HEAD"]);
    choose(&mut h, "Add main", "Reset current branch to this Commit...");
    // Reset asks for the mode first, then confirms.
    if matches!(h.state.dialog, Some(Dialog::Input { .. })) {
        press(&mut h, KeyCode::Enter);
    }
    assert!(
        matches!(h.state.dialog, Some(Dialog::Confirm { .. })),
        "{:?}",
        h.state.dialog.is_some()
    );
    press(&mut h, KeyCode::Esc);
    assert!(h.state.dialog.is_none());
    assert_eq!(repo.git(&["rev-parse", "HEAD"]), head);
}

#[test]
fn deleting_an_unmerged_branch_offers_a_force_delete() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    choose(&mut h, "feature/login", "Delete Branch...");
    if matches!(h.state.dialog, Some(Dialog::Input { .. })) {
        press(&mut h, KeyCode::Enter); // keep "force" off
    }
    // Destructive: focus starts on Cancel, so move to Confirm first.
    assert!(matches!(h.state.dialog, Some(Dialog::Confirm { .. })));
    press(&mut h, KeyCode::Left);
    press(&mut h, KeyCode::Enter);
    // `branch -d` refuses: the app offers a force delete instead.
    assert!(
        matches!(h.state.dialog, Some(Dialog::Confirm { .. })),
        "force delete offered"
    );
    assert!(
        !repo.git(&["branch", "--list", "feature/login"]).is_empty(),
        "not deleted yet"
    );
    press(&mut h, KeyCode::Left);
    press(&mut h, KeyCode::Enter);
    assert!(
        repo.git(&["branch", "--list", "feature/login"]).is_empty(),
        "force deleted"
    );
    assert!(
        h.state
            .refs
            .iter()
            .all(|r| r.short_name() != "feature/login")
    );
}

#[test]
fn copy_actions_go_to_the_clipboard() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    let hash = repo.git(&["log", "--all", "--format=%H", "--grep=Update docs"]);
    choose(&mut h, "Update docs", "Copy Commit Hash to Clipboard");
    assert_eq!(h.clipboard, vec![hash]);
    assert!(
        h.state
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("Copied")
    );
}

#[test]
fn ui_only_actions_show_a_hint_and_run_no_git() {
    let repo = FixtureRepo::sample();
    let before = repo.git(&["rev-parse", "HEAD"]);

    let mut h = open(&repo);
    choose(&mut h, "v0.1.0", "View Details");
    let message = h.state.message.clone().unwrap_or_default();
    assert!(message.contains("v0.1.0"), "{message}");
    assert!(h.state.running.is_none());

    choose(&mut h, "Uncommitted Changes", "Open Source Control View");
    let message = h.state.message.clone().unwrap_or_default();
    assert!(message.contains("source control"), "{message}");
    assert!(h.state.running.is_none());
    assert_eq!(repo.git(&["rev-parse", "HEAD"]), before);
}

#[test]
fn git_failures_are_reported_and_the_view_refreshes() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    // A local change to a file that differs on develop blocks the checkout.
    repo.write_file("src/lib.rs", "pub fn local_edit() {}\n");
    choose(&mut h, "develop", "Checkout Branch");
    assert_eq!(current_branch(&repo), "main");
    let message = h.state.message.clone().unwrap_or_default();
    assert!(
        message.contains("uncommitted changes would be overwritten"),
        "{message}"
    );
}

#[test]
fn refreshing_does_not_cancel_a_running_action() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    let (x, y) = h.find_on_screen("develop").unwrap();
    click(&mut h, MouseButton::Right, x + 1, y);
    let (x, y) = h.find_on_screen("Checkout Branch").unwrap();
    // Choose the action and press `r` before any git result arrives.
    let mut commands = h.state.update(mouse_down(MouseButton::Left, x, y));
    commands.extend(h.state.update(Event::Key(KeyEvent::new(
        KeyCode::Char('r'),
        KeyModifiers::NONE,
    ))));
    h.settle(commands);
    assert_eq!(current_branch(&repo), "develop");
}

#[test]
fn a_second_action_waits_for_the_first() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    let (x, y) = h.find_on_screen("develop").unwrap();
    click(&mut h, MouseButton::Right, x + 1, y);
    let (x, y) = h.find_on_screen("Checkout Branch").unwrap();
    let first = h.state.update(mouse_down(MouseButton::Left, x, y));
    assert!(!first.is_empty());
    // While it runs, starting another action is refused with a message.
    h.state.start_action(
        tgg_actions::Action::CopyBranchName { name: "x".into() },
        "Copy".into(),
    );
    assert!(
        h.state
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("Wait")
    );
    h.settle(first);
}
