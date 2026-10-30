//! "Show Reflog" (S20 4): where HEAD has been, and recovering a commit a
//! reset left on no branch, against real fixture repositories.

use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ggt_actions::{Action, UiEffect};
use ggt_testkit::FixtureRepo;
use pretty_assertions::assert_eq;

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

fn click_text(h: &mut Harness, button: MouseButton, text: &str) {
    let (x, y) = find(h, text);
    mouse(h, MouseEventKind::Down(button), x, y);
}

/// Opens "Show Reflog" from the "..." in the Source Control pane's header.
fn show_reflog(h: &mut Harness) {
    let (x, y) = find(h, "✓ ↻ …");
    mouse(h, MouseEventKind::Down(MouseButton::Left), x + 4, y);
    click_text(h, MouseButton::Left, "Show Reflog");
}

/// The sample repository with one more commit on `main`, then reset away.
/// Returns the lost commit's hash.
fn lose_a_commit(repo: &FixtureRepo) -> String {
    let lost = repo.commit_file("lost.txt", "lost\n", "Lost work");
    repo.git(&["reset", "--hard", "HEAD~1"]);
    lost
}

fn selected_subject(h: &Harness) -> Option<&str> {
    let view = h.state.reflog.as_ref()?;
    view.entries.get(view.selected).map(|e| e.subject.as_str())
}

#[test]
fn the_reflog_lists_where_head_has_been() {
    let repo = FixtureRepo::sample();
    let lost = lose_a_commit(&repo);
    let mut h = open(&repo);
    show_reflog(&mut h);

    find(&h, " reflog · enter actions · esc closes ");
    let (_, reset_y) = find(&h, "reset: moving to HEAD~1");
    let (_, lost_y) = find(&h, "commit: Lost work");
    assert!(reset_y < lost_y, "newest first");
    find(&h, &lost[..7]);
    find(&h, "HEAD@{1}");
    assert_eq!(selected_subject(&h), Some("reset: moving to HEAD~1"));
}

#[test]
fn keys_and_clicks_move_the_selection() {
    let repo = FixtureRepo::sample();
    lose_a_commit(&repo);
    let mut h = open(&repo);
    show_reflog(&mut h);

    press(&mut h, KeyCode::Char('j'));
    assert_eq!(selected_subject(&h), Some("commit: Lost work"));
    press(&mut h, KeyCode::Up);
    assert_eq!(selected_subject(&h), Some("reset: moving to HEAD~1"));
    press(&mut h, KeyCode::End);
    let last = h.state.reflog.as_ref().unwrap().entries.len() - 1;
    assert_eq!(h.state.reflog.as_ref().unwrap().selected, last);
    press(&mut h, KeyCode::Home);
    assert_eq!(h.state.reflog.as_ref().unwrap().selected, 0);

    click_text(&mut h, MouseButton::Left, "commit: Lost work");
    assert_eq!(selected_subject(&h), Some("commit: Lost work"));
    assert!(h.state.menu.is_none(), "a left click only selects");
}

#[test]
fn the_mouse_wheel_scrolls_the_entries() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    show_reflog(&mut h);
    // A short terminal, so the entries don't all fit.
    let short = 12;
    h.draw(W, short);
    let (x, y) = find(&h, "HEAD@{0}");
    let wheel = |h: &mut Harness, kind| {
        let commands = h.state.update(Event::Mouse(MouseEvent {
            kind,
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        }));
        h.settle(commands);
        h.draw(W, short);
    };
    wheel(&mut h, MouseEventKind::ScrollDown);
    assert_eq!(h.state.reflog.as_ref().unwrap().offset, SCROLL_LINES);
    assert!(h.find_on_screen("HEAD@{0}").is_none(), "scrolled off");
    wheel(&mut h, MouseEventKind::ScrollUp);
    assert_eq!(h.state.reflog.as_ref().unwrap().offset, 0);
}

#[test]
fn create_branch_here_recovers_a_lost_commit() {
    let repo = FixtureRepo::sample();
    let lost = lose_a_commit(&repo);
    let mut h = open(&repo);
    show_reflog(&mut h);

    press(&mut h, KeyCode::Down);
    assert_eq!(selected_subject(&h), Some("commit: Lost work"));
    press(&mut h, KeyCode::Enter);
    click_text(&mut h, MouseButton::Left, "Create Branch Here...");
    assert!(
        matches!(h.state.dialog, Some(Dialog::Input { .. })),
        "asks for the branch name"
    );
    type_text(&mut h, "rescued");
    press(&mut h, KeyCode::Enter);

    assert!(h.state.dialog.is_none());
    assert_eq!(repo.git(&["rev-parse", "rescued"]), lost);
    assert!(
        h.state.commits.iter().any(|c| c.subject == "Lost work"),
        "the recovered commit is back in the graph"
    );
    assert!(h.state.reflog.is_some(), "the reflog stays open");
    assert_eq!(selected_subject(&h), Some("commit: Lost work"));
}

#[test]
fn a_right_click_opens_the_entry_menu() {
    let repo = FixtureRepo::sample();
    let lost = lose_a_commit(&repo);
    let mut h = open(&repo);
    show_reflog(&mut h);

    click_text(&mut h, MouseButton::Right, "commit: Lost work");
    click_text(&mut h, MouseButton::Left, "Copy Commit Hash to Clipboard");
    assert_eq!(h.clipboard, [lost]);
}

#[test]
fn checkout_reloads_the_reflog_with_the_new_entry() {
    let repo = FixtureRepo::sample();
    let lost = lose_a_commit(&repo);
    let mut h = open(&repo);
    show_reflog(&mut h);

    click_text(&mut h, MouseButton::Right, "commit: Lost work");
    click_text(&mut h, MouseButton::Left, "Checkout (detached)...");
    // Confirm the detached checkout.
    press(&mut h, KeyCode::Enter);

    assert_eq!(repo.git(&["rev-parse", "HEAD"]), lost);
    let view = h.state.reflog.as_ref().unwrap();
    assert!(
        view.entries[0]
            .subject
            .starts_with("checkout: moving from main to"),
        "{:?}",
        view.entries[0]
    );
    assert_eq!(selected_subject(&h), Some("commit: Lost work"), "kept");
}

#[test]
fn escape_closes_the_reflog_and_keys_go_back_to_the_graph() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    let before = h.state.selected;
    // Opened without clicking the pane, which would give the pane the keys.
    let commands = h
        .state
        .start_action(Action::Ui(UiEffect::ShowReflog), "Show Reflog".into());
    h.settle(commands);
    h.draw(W, H);
    assert!(h.state.reflog.is_some());
    press(&mut h, KeyCode::Char('j'));
    assert_eq!(h.state.selected, before, "the graph doesn't move");

    press(&mut h, KeyCode::Esc);
    assert!(h.state.reflog.is_none());
    assert!(h.find_on_screen(" reflog · ").is_none());
    assert!(!h.state.should_quit, "Esc only closed the panel");
    press(&mut h, KeyCode::Char('j'));
    assert_eq!(h.state.selected, before + 1);
}
