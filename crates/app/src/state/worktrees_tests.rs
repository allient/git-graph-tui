//! "Worktrees..." (S20 4): listing, adding, removing and pruning worktrees,
//! against real fixture repositories. Worktree folders go in their own
//! temporary directory, deleted with it.

use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tgg_actions::{Action, UiEffect};
use tgg_testkit::FixtureRepo;

use super::*;
use crate::testing::Harness;

const W: u16 = 140;
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

/// Opens "Worktrees..." from the "..." in the Source Control pane's header.
fn show_worktrees(h: &mut Harness) {
    let (x, y) = find(h, "✓ ↻ …");
    mouse(h, MouseEventKind::Down(MouseButton::Left), x + 4, y);
    click_text(h, MouseButton::Left, "Worktrees...");
}

/// Checks out `branch` in a worktree at `scratch/<name>`, with git itself.
fn add_worktree(repo: &FixtureRepo, scratch: &TempDir, name: &str, branch: &str) -> PathBuf {
    let path = scratch.path().join(name);
    repo.git(&["worktree", "add", "--quiet", path.to_str().unwrap(), branch]);
    path
}

/// `git worktree list`, as the folder names.
fn listed(repo: &FixtureRepo) -> Vec<String> {
    repo.git(&["worktree", "list", "--porcelain"])
        .lines()
        .filter_map(|l| l.strip_prefix("worktree "))
        .map(|p| folder(Path::new(p)))
        .collect()
}

fn folder(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().into_owned()
}

fn selected_folder(h: &Harness) -> Option<String> {
    let entry = h.state.worktrees.as_ref()?.selected_entry()?;
    Some(folder(Path::new(&entry.worktree.path)))
}

/// Selects the worktree in folder `name` and opens its menu.
fn menu_of(h: &mut Harness, name: &str) {
    click_text(h, MouseButton::Right, name);
    assert_eq!(selected_folder(h).as_deref(), Some(name));
    assert!(h.state.menu.is_some(), "the entry's menu opens");
}

#[test]
fn the_view_lists_the_worktrees_and_marks_main_and_current() {
    let repo = FixtureRepo::sample();
    let scratch = TempDir::new().unwrap();
    add_worktree(&repo, &scratch, "wt-develop", "develop");
    let mut h = open(&repo);
    show_worktrees(&mut h);

    find(&h, " worktrees · enter actions · a add · esc closes ");
    let view = h.state.worktrees.as_ref().unwrap();
    assert_eq!(view.entries.len(), 2, "{:?}", view.entries);
    assert!(view.entries[0].is_current && view.entries[0].worktree.is_main);
    assert!(!view.entries[1].is_current);
    let main_row = find(&h, "main ").1;
    let (_, dev_row) = find(&h, "wt-develop");
    assert!(main_row < dev_row, "the main worktree comes first");
    find(&h, "develop");
    find(&h, "current · main");
    let develop = repo.git(&["rev-parse", "--short=7", "develop"]);
    find(&h, &develop);
}

#[test]
fn add_creates_a_worktree_on_a_new_branch() {
    let repo = FixtureRepo::sample();
    let scratch = TempDir::new().unwrap();
    let target = scratch.path().join("wt-topic");
    let mut h = open(&repo);
    show_worktrees(&mut h);

    press(&mut h, KeyCode::Char('a'));
    assert!(
        matches!(h.state.dialog, Some(Dialog::Input { .. })),
        "asks for the branch and folder"
    );
    type_text(&mut h, "topic");
    press(&mut h, KeyCode::Tab); // "Create a new branch" is on.
    press(&mut h, KeyCode::Tab);
    type_text(&mut h, target.to_str().unwrap());
    press(&mut h, KeyCode::Enter);

    assert!(h.state.dialog.is_none(), "{:?}", h.state.message);
    assert!(listed(&repo).contains(&"wt-topic".to_owned()));
    assert_eq!(
        repo.git(&["rev-parse", "topic"]),
        repo.git(&["rev-parse", "main"]),
        "the new branch starts at HEAD"
    );
    let view = h.state.worktrees.as_ref().unwrap();
    assert!(
        view.entries
            .iter()
            .any(|e| e.worktree.branch.as_deref() == Some("topic")),
        "the view reloads with it"
    );
    assert!(
        h.state.refs.iter().any(|r| r.short_name() == "topic"),
        "the graph refreshes with the branch"
    );
}

#[test]
fn add_checks_out_an_existing_branch() {
    let repo = FixtureRepo::sample();
    let scratch = TempDir::new().unwrap();
    let target = scratch.path().join("wt-develop");
    let mut h = open(&repo);
    show_worktrees(&mut h);
    // The main worktree is selected; its menu adds one too.
    press(&mut h, KeyCode::Enter);
    click_text(&mut h, MouseButton::Left, "Add Worktree...");

    type_text(&mut h, "develop");
    press(&mut h, KeyCode::Tab);
    press(&mut h, KeyCode::Char(' ')); // An existing branch.
    press(&mut h, KeyCode::Tab);
    type_text(&mut h, target.to_str().unwrap());
    press(&mut h, KeyCode::Enter);

    assert!(h.state.dialog.is_none(), "{:?}", h.state.message);
    let target = target.to_str().unwrap();
    assert_eq!(
        repo.git(&["-C", target, "symbolic-ref", "--short", "HEAD"]),
        "develop"
    );
    assert!(listed(&repo).contains(&"wt-develop".to_owned()));
}

#[test]
fn remove_asks_first_and_deletes_the_folder() {
    let repo = FixtureRepo::sample();
    let scratch = TempDir::new().unwrap();
    let path = {
        repo.git(&["branch", "old", "develop"]);
        add_worktree(&repo, &scratch, "wt-old", "old")
    };
    let mut h = open(&repo);
    show_worktrees(&mut h);

    menu_of(&mut h, "wt-old");
    click_text(&mut h, MouseButton::Left, "Remove Worktree...");
    assert!(matches!(h.state.dialog, Some(Dialog::Input { .. })));
    press(&mut h, KeyCode::Enter); // Not forced.
    assert!(
        matches!(h.state.dialog, Some(Dialog::Confirm { .. })),
        "removing a worktree is confirmed first"
    );
    assert!(path.exists(), "nothing is deleted before confirming");
    // Destructive: focus starts on Cancel.
    press(&mut h, KeyCode::Left);
    press(&mut h, KeyCode::Enter);

    assert!(!path.exists(), "the folder is deleted");
    assert!(!listed(&repo).contains(&"wt-old".to_owned()));
    let view = h.state.worktrees.as_ref().unwrap();
    assert_eq!(view.entries.len(), 1, "the view reloads without it");
    assert_eq!(
        repo.git(&["branch", "--list", "old"]),
        "  old",
        "the branch stays"
    );
}

#[test]
fn the_main_and_the_current_worktree_cannot_be_removed() {
    let repo = FixtureRepo::sample();
    let scratch = TempDir::new().unwrap();
    let linked = add_worktree(&repo, &scratch, "wt-here", "develop");
    let mut h = open(&repo);
    // The app runs in the linked worktree.
    h.state.repo.clone_from(&linked);
    let commands = h.state.update(Event::Refresh);
    h.settle(commands);
    h.draw(W, H);
    let commands = h
        .state
        .start_action(Action::Ui(UiEffect::ShowWorktrees), "Worktrees".into());
    h.settle(commands);
    h.draw(W, H);

    let view = h.state.worktrees.as_ref().unwrap();
    assert!(view.entries[0].worktree.is_main && !view.entries[0].is_current);
    assert!(view.entries[1].is_current);

    press(&mut h, KeyCode::Enter);
    click_text(&mut h, MouseButton::Left, "Remove Worktree...");
    assert!(h.state.dialog.is_none());
    assert_eq!(
        h.state.message.as_deref(),
        Some("The main worktree can't be removed.")
    );
    press(&mut h, KeyCode::Esc);

    menu_of(&mut h, "wt-here");
    click_text(&mut h, MouseButton::Left, "Remove Worktree...");
    assert!(h.state.dialog.is_none());
    assert_eq!(
        h.state.message.as_deref(),
        Some("tui-git-graph is running in this worktree.")
    );
    assert!(linked.exists());
}

#[test]
fn a_dirty_worktree_needs_force() {
    let repo = FixtureRepo::sample();
    let scratch = TempDir::new().unwrap();
    let path = add_worktree(&repo, &scratch, "wt-dirty", "develop");
    std::fs::write(path.join("scratch.txt"), "untracked\n").unwrap();
    let mut h = open(&repo);
    show_worktrees(&mut h);

    let remove = |h: &mut Harness, force: bool| {
        menu_of(h, "wt-dirty");
        click_text(h, MouseButton::Left, "Remove Worktree...");
        if force {
            press(h, KeyCode::Char(' '));
        }
        press(h, KeyCode::Enter);
        press(h, KeyCode::Left);
        press(h, KeyCode::Enter);
    };
    remove(&mut h, false);
    assert!(path.exists(), "git refuses without force");
    let message = h.state.message.clone().unwrap_or_default();
    assert!(message.contains("failed"), "{message}");

    remove(&mut h, true);
    assert!(!path.exists(), "{:?}", h.state.message);
    assert!(!listed(&repo).contains(&"wt-dirty".to_owned()));
}

#[test]
fn a_locked_worktree_unlocks_and_force_removes() {
    let repo = FixtureRepo::sample();
    let scratch = TempDir::new().unwrap();
    let path = add_worktree(&repo, &scratch, "wt-usb", "develop");
    let mut h = open(&repo);
    show_worktrees(&mut h);

    menu_of(&mut h, "wt-usb");
    click_text(&mut h, MouseButton::Left, "Lock");
    let porcelain = repo.git(&["worktree", "list", "--porcelain"]);
    assert!(porcelain.contains("\nlocked"), "{porcelain}");
    find(&h, "locked");

    menu_of(&mut h, "wt-usb");
    click_text(&mut h, MouseButton::Left, "Remove Worktree...");
    press(&mut h, KeyCode::Char(' ')); // Force.
    press(&mut h, KeyCode::Enter);
    press(&mut h, KeyCode::Left);
    press(&mut h, KeyCode::Enter);
    assert!(!path.exists(), "{:?}", h.state.message);
}

#[test]
fn prune_forgets_a_worktree_whose_folder_is_gone() {
    let repo = FixtureRepo::sample();
    let scratch = TempDir::new().unwrap();
    let path = add_worktree(&repo, &scratch, "wt-gone", "develop");
    std::fs::remove_dir_all(&path).unwrap();
    let mut h = open(&repo);
    show_worktrees(&mut h);
    find(&h, "prunable");

    menu_of(&mut h, "wt-gone");
    click_text(&mut h, MouseButton::Left, "Prune Worktrees");
    assert!(!listed(&repo).contains(&"wt-gone".to_owned()));
    assert_eq!(h.state.worktrees.as_ref().unwrap().entries.len(), 1);
}

#[test]
fn copy_path_puts_the_folder_on_the_clipboard() {
    let repo = FixtureRepo::sample();
    let scratch = TempDir::new().unwrap();
    add_worktree(&repo, &scratch, "wt-copy", "develop");
    let mut h = open(&repo);
    show_worktrees(&mut h);

    menu_of(&mut h, "wt-copy");
    let path = h
        .state
        .worktrees
        .as_ref()
        .unwrap()
        .selected_entry()
        .unwrap()
        .worktree
        .path
        .clone();
    click_text(&mut h, MouseButton::Left, "Copy Path to Clipboard");
    assert_eq!(h.clipboard, [path]);
}

#[test]
fn escape_closes_the_view_and_keys_go_back_to_the_graph() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    let before = h.state.selected;
    let commands = h
        .state
        .start_action(Action::Ui(UiEffect::ShowWorktrees), "Worktrees".into());
    h.settle(commands);
    h.draw(W, H);
    assert!(h.state.worktrees.is_some());
    press(&mut h, KeyCode::Char('j'));
    assert_eq!(h.state.selected, before, "the graph doesn't move");

    press(&mut h, KeyCode::Esc);
    assert!(h.state.worktrees.is_none());
    assert!(h.find_on_screen(" worktrees · ").is_none());
    assert!(!h.state.should_quit, "Esc only closed the panel");
    press(&mut h, KeyCode::Char('j'));
    assert_eq!(h.state.selected, before + 1);
}
