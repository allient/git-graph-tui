//! "Show Git Output": the log of the git commands actions ran.

use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use pretty_assertions::assert_eq;
use tgg_actions::{Action, UiEffect};
use tgg_testkit::FixtureRepo;

use super::output::{OUTPUT_CAP, OutputEntry};
use super::*;
use crate::testing::Harness;

const W: u16 = 110;
const H: u16 = 24;

fn open(repo: &FixtureRepo) -> Harness {
    let mut h = Harness::open(repo);
    h.draw(W, H);
    h
}

fn run(h: &mut Harness, action: Action) {
    let commands = h.state.start_action(action, "Test".into());
    h.settle(commands);
    h.draw(W, H);
}

fn show_output(h: &mut Harness) {
    run(h, Action::Ui(UiEffect::ShowGitOutput));
}

#[test]
fn actions_are_recorded_with_their_status_but_background_reads_are_not() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    assert!(
        h.state.git_output.entries().is_empty(),
        "refresh reads aren't logged"
    );

    run(
        &mut h,
        Action::CheckoutBranch {
            name: "develop".into(),
        },
    );
    // Blocked: a local edit to a file that differs on feature/login.
    repo.write_file("src/login.rs", "local\n");
    run(
        &mut h,
        Action::CheckoutBranch {
            name: "feature/login".into(),
        },
    );

    let entries = h.state.git_output.entries();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].command, "git switch -- develop");
    assert_eq!(entries[0].status, "ok");
    assert_eq!(entries[1].command, "git switch -- feature/login");
    assert_eq!(entries[1].status, "exit 1");
    assert!(
        entries[1].stderr.contains("would be overwritten"),
        "{}",
        entries[1].stderr
    );
}

#[test]
fn the_view_lists_the_newest_command_first_and_esc_closes_it() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    run(
        &mut h,
        Action::CheckoutBranch {
            name: "develop".into(),
        },
    );
    run(
        &mut h,
        Action::CheckoutBranch {
            name: "main".into(),
        },
    );
    show_output(&mut h);
    assert!(h.state.output_view.is_some());
    let newest = h.find_on_screen("$ git switch -- main").expect("drawn");
    let older = h.find_on_screen("$ git switch -- develop").expect("drawn");
    assert!(newest.1 < older.1, "newest first");
    h.key(KeyCode::Esc);
    assert!(h.state.output_view.is_none());
}

#[test]
fn an_empty_log_says_so() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    show_output(&mut h);
    assert!(h.find_on_screen("No git commands have run yet.").is_some());
}

#[test]
fn the_log_keeps_the_newest_entries_only() {
    let mut log = GitOutput::default();
    for i in 0..OUTPUT_CAP + 5 {
        log.push(OutputEntry {
            command: format!("git {i}"),
            status: "ok".into(),
            stdout: String::new(),
            stderr: String::new(),
        });
    }
    assert_eq!(log.entries().len(), OUTPUT_CAP);
    assert_eq!(log.entries()[0].command, "git 5");
}

#[test]
fn keys_and_the_wheel_scroll_within_the_output() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    for _ in 0..6 {
        run(
            &mut h,
            Action::CheckoutBranch {
                name: "develop".into(),
            },
        );
        run(
            &mut h,
            Action::CheckoutBranch {
                name: "main".into(),
            },
        );
    }
    show_output(&mut h);
    let view = |h: &Harness| h.state.output_view.as_ref().unwrap().offset;
    h.key(KeyCode::Char('j'));
    assert_eq!(view(&h), 1);
    h.key(KeyCode::End);
    h.draw(W, H);
    let bottom = view(&h);
    assert!(bottom > 1);
    h.key(KeyCode::Down);
    assert_eq!(view(&h), bottom, "clamped at the end");
    let commands = h.state.update(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: 5,
        row: 5,
        modifiers: KeyModifiers::NONE,
    }));
    h.settle(commands);
    assert_eq!(view(&h), bottom - SCROLL_LINES);
    h.key(KeyCode::Home);
    assert_eq!(view(&h), 0);
    let commands = h.state.update(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: 5,
        row: 5,
        modifiers: KeyModifiers::NONE,
    }));
    h.settle(commands);
    assert!(h.state.output_view.is_none(), "right click closes");
}
