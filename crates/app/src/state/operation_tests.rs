//! The conflict banner against real conflicts: detection, Continue, Abort
//! and the actions that wait for the operation.

use std::process::Command as Process;

use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use pretty_assertions::assert_eq;
use tgg_actions::{Action, Operation, WorkingTreeAction};
use tgg_testkit::FixtureRepo;

use super::*;
use crate::testing::Harness;

const W: u16 = 110;
const H: u16 = 24;

/// `main` and `topic` both change the only line of `file.txt`.
fn diverged() -> FixtureRepo {
    let repo = FixtureRepo::new();
    repo.commit_file("file.txt", "base\n", "Base");
    repo.branch("topic");
    repo.commit_file("file.txt", "topic\n", "Topic edit");
    repo.checkout("main");
    repo.commit_file("file.txt", "main\n", "Main edit");
    repo
}

/// Runs a git command that stops on a conflict (and so fails).
fn conflict(repo: &FixtureRepo, args: &[&str]) {
    let output = Process::new("git")
        .args(args)
        .current_dir(repo.path())
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_EDITOR", "true")
        .output()
        .expect("start git");
    assert!(
        !output.status.success(),
        "git {} did not conflict",
        args.join(" ")
    );
}

/// A merge of `topic` into `main` stopped on a conflict in `file.txt`.
fn merging() -> FixtureRepo {
    let repo = diverged();
    conflict(&repo, &["merge", "topic"]);
    repo
}

fn open(repo: &FixtureRepo) -> Harness {
    let mut h = Harness::open(repo);
    h.draw(W, H);
    h
}

fn press(h: &mut Harness, code: KeyCode) {
    h.key(code);
    h.draw(W, H);
}

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

fn click_text(h: &mut Harness, button: MouseButton, text: &str) {
    let (x, y) = h
        .find_on_screen(text)
        .unwrap_or_else(|| panic!("{text:?} not on screen"));
    click(h, button, x + 1, y);
}

fn merge_head(repo: &FixtureRepo) -> bool {
    repo.path().join(".git/MERGE_HEAD").exists()
}

/// Everything a refused action could have changed.
fn data_hash(repo: &FixtureRepo) -> String {
    [
        repo.git(&["rev-parse", "HEAD"]),
        repo.git(&["symbolic-ref", "HEAD"]),
        repo.git_raw(&["status", "--porcelain=v2"]),
        repo.git(&["ls-files", "--stage"]),
        std::fs::read_to_string(repo.path().join(".git/MERGE_HEAD")).unwrap_or_default(),
        std::fs::read_to_string(repo.path().join("file.txt")).unwrap_or_default(),
    ]
    .join("\n")
}

fn resolve(repo: &FixtureRepo) {
    repo.write_file("file.txt", "both\n");
    repo.git(&["add", "--", "file.txt"]);
}

fn message(h: &Harness) -> &str {
    h.state.message.as_deref().unwrap_or_default()
}

#[test]
fn a_conflicted_merge_shows_the_banner() {
    let repo = merging();
    let h = open(&repo);
    assert_eq!(h.state.operation, Some(Operation::Merge));
    assert!(
        h.find_on_screen("Merge in progress · 1 conflicted file   [Continue] [Abort]")
            .is_some(),
        "banner on screen"
    );
}

#[test]
fn no_banner_without_an_operation() {
    let repo = diverged();
    let h = open(&repo);
    assert_eq!(h.state.operation, None);
    assert!(h.find_on_screen("[Continue]").is_none());
}

#[test]
fn a_conflicted_rebase_shows_rebase_in_progress() {
    let repo = diverged();
    conflict(&repo, &["rebase", "topic"]);
    let h = open(&repo);
    assert_eq!(h.state.operation, Some(Operation::Rebase));
    assert!(
        h.find_on_screen("Rebase in progress · 1 conflicted file")
            .is_some()
    );
}

#[test]
fn continue_is_refused_while_files_are_conflicted() {
    let repo = merging();
    let mut h = open(&repo);
    let before = data_hash(&repo);
    press(&mut h, KeyCode::Char('C'));
    assert_eq!(
        message(&h),
        "Resolve the conflicted file and stage it first."
    );
    assert!(h.state.dialog.is_none());
    assert_eq!(data_hash(&repo), before, "nothing ran");
}

#[test]
fn continue_finishes_the_merge_once_conflicts_are_resolved() {
    let repo = merging();
    let mut h = open(&repo);
    resolve(&repo);
    press(&mut h, KeyCode::Char('r'));
    assert!(
        h.find_on_screen("Merge in progress · no conflicts left")
            .is_some(),
        "banner after resolving"
    );
    press(&mut h, KeyCode::Char('C'));
    assert_eq!(message(&h), "✓ Continue Merge");
    assert!(!merge_head(&repo), "merge finished");
    let parents = repo.git(&["log", "-1", "--format=%P"]);
    assert_eq!(parents.split(' ').count(), 2, "a merge commit: {parents}");
    assert_eq!(
        repo.git(&["log", "-1", "--format=%s"]),
        "Merge branch 'topic'"
    );
    assert_eq!(h.state.operation, None);
    assert!(h.find_on_screen("[Continue]").is_none(), "banner gone");
}

#[test]
fn abort_asks_first_then_restores_the_branch() {
    let repo = merging();
    let head = repo.git(&["rev-parse", "HEAD"]);
    let mut h = open(&repo);
    press(&mut h, KeyCode::Char('A'));
    let Some(Dialog::Confirm { view, .. }) = &h.state.dialog else {
        panic!("Abort asks for confirmation");
    };
    assert_eq!(view.confirmation().title, "Abort Merge");
    assert!(merge_head(&repo), "not aborted before confirming");
    // Destructive: focus starts on Cancel.
    press(&mut h, KeyCode::Left);
    press(&mut h, KeyCode::Enter);
    assert!(!merge_head(&repo), "merge aborted");
    assert_eq!(
        repo.git(&["status", "--porcelain"]),
        "",
        "working tree clean"
    );
    assert_eq!(repo.git(&["rev-parse", "HEAD"]), head);
    assert_eq!(h.state.operation, None);
}

#[test]
fn the_banner_buttons_are_clickable() {
    let repo = merging();
    let mut h = open(&repo);
    click_text(&mut h, MouseButton::Left, "[Continue]");
    assert_eq!(
        message(&h),
        "Resolve the conflicted file and stage it first."
    );
    click_text(&mut h, MouseButton::Left, "[Abort]");
    assert!(
        matches!(h.state.dialog, Some(Dialog::Confirm { .. })),
        "Abort asks first"
    );
    press(&mut h, KeyCode::Esc);
    assert!(merge_head(&repo), "cancelled");
}

#[test]
fn a_checkout_waits_for_the_merge() {
    let repo = merging();
    let mut h = open(&repo);
    let before = data_hash(&repo);
    click_text(&mut h, MouseButton::Right, "topic");
    click_text(&mut h, MouseButton::Left, "Checkout Branch");
    assert_eq!(
        message(&h),
        "A merge is in progress: continue or abort it first."
    );
    assert!(h.state.dialog.is_none());
    assert_eq!(data_hash(&repo), before, "nothing changed");
}

#[test]
fn staging_a_resolution_stays_allowed() {
    let repo = merging();
    let mut h = open(&repo);
    repo.write_file("file.txt", "both\n");
    let stage = Action::WorkingTree(WorkingTreeAction::Stage {
        paths: vec!["file.txt".into()],
    });
    let commands = h.state.start_action(stage, "Stage".into());
    h.settle(commands);
    h.draw(W, H);
    assert_eq!(message(&h), "✓ Stage");
    assert!(
        h.find_on_screen("Merge in progress · no conflicts left")
            .is_some()
    );
}
