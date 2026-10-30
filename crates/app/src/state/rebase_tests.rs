//! "Interactive Rebase from Here..." (S20 4) against real fixture
//! repositories: opening the panel, reordering, squashing, fixing up and
//! dropping commits, cancelling, and the conflict banner taking over.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use pretty_assertions::assert_eq;
use tgg_actions::{Operation, TodoAction};
use tgg_testkit::FixtureRepo;

use super::*;
use crate::testing::Harness;

const W: u16 = 110;
const H: u16 = 30;

/// `main`: alpha, bravo, charlie, delta, each adding its own file.
fn linear() -> FixtureRepo {
    let repo = FixtureRepo::new();
    repo.commit_file("alpha.txt", "alpha\n", "Add alpha");
    repo.commit_file("bravo.txt", "bravo\n", "Add bravo");
    repo.commit_file("charlie.txt", "charlie\n", "Add charlie");
    repo.commit_file("delta.txt", "delta\n", "Add delta");
    repo
}

/// `main`: alpha, bravo, then charlie and delta editing the same line of
/// `shared.txt` one after the other, so delta can't go before charlie.
fn dependent() -> FixtureRepo {
    let repo = FixtureRepo::new();
    repo.commit_file("alpha.txt", "alpha\n", "Add alpha");
    repo.commit_file("shared.txt", "bravo\n", "Add bravo");
    repo.commit_file("shared.txt", "charlie\n", "Add charlie");
    repo.commit_file("shared.txt", "delta\n", "Add delta");
    repo
}

fn open(repo: &FixtureRepo) -> Harness {
    let mut h = Harness::open(repo);
    h.draw(W, H);
    h
}

fn key_with(h: &mut Harness, code: KeyCode, modifiers: KeyModifiers) {
    let commands = h.state.update(Event::Key(KeyEvent::new(code, modifiers)));
    h.settle(commands);
    h.draw(W, H);
}

fn press(h: &mut Harness, code: KeyCode) {
    key_with(h, code, KeyModifiers::NONE);
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

fn find(h: &Harness, text: &str) -> (u16, u16) {
    h.find_on_screen(text)
        .unwrap_or_else(|| panic!("{text:?} not on screen"))
}

/// Right-clicks the commit row showing `subject` and picks `entry`.
fn choose(h: &mut Harness, subject: &str, entry: &str) {
    let (x, y) = find(h, subject);
    click(h, MouseButton::Right, x + 1, y);
    let (x, y) = find(h, entry);
    click(h, MouseButton::Left, x, y);
}

/// Opens the panel from bravo's menu: it edits charlie and delta.
fn open_panel(h: &mut Harness) {
    choose(h, "Add bravo", "Interactive Rebase from Here...");
    assert!(
        h.state.rebase.is_some(),
        "panel open: {:?}",
        h.state.message
    );
}

/// Enter, then confirms the destructive dialog (focus starts on Cancel).
fn start_and_confirm(h: &mut Harness) {
    press(h, KeyCode::Enter);
    let Some(Dialog::Confirm { view, .. }) = &h.state.dialog else {
        panic!("Enter asks for confirmation: {:?}", h.state.message);
    };
    assert_eq!(view.confirmation().title, "Interactive Rebase");
    press(h, KeyCode::Left);
    press(h, KeyCode::Enter);
}

fn subjects(repo: &FixtureRepo) -> String {
    repo.git(&["log", "--format=%s"])
}

fn tree(repo: &FixtureRepo) -> String {
    repo.git(&["rev-parse", "HEAD^{tree}"])
}

/// Everything a refused or cancelled rebase could have changed.
fn data_hash(repo: &FixtureRepo) -> String {
    [
        repo.git(&["rev-parse", "HEAD"]),
        repo.git(&["symbolic-ref", "HEAD"]),
        repo.git_raw(&["status", "--porcelain=v2", "--untracked-files=all"]),
        repo.git(&["ls-files", "--stage"]),
        repo.git(&["for-each-ref"]),
        repo.path().join(".git/rebase-merge").exists().to_string(),
    ]
    .join("\n")
}

fn panel_subjects(h: &Harness) -> Vec<String> {
    h.state
        .rebase
        .as_ref()
        .map(|v| v.rebase.items.iter().map(|i| i.subject.clone()).collect())
        .unwrap_or_default()
}

fn message(h: &Harness) -> &str {
    h.state.message.as_deref().unwrap_or_default()
}

#[test]
fn the_panel_lists_the_commits_after_the_base_oldest_first() {
    let repo = linear();
    let bravo = repo.git(&["rev-parse", "--short=7", "HEAD~2"]);
    let mut h = open(&repo);
    open_panel(&mut h);
    assert_eq!(panel_subjects(&h), ["Add charlie", "Add delta"]);
    find(
        &h,
        &format!(" interactive rebase onto {bravo} · enter start · esc cancel "),
    );
    let (_, charlie) = find(&h, "Add charlie");
    let (_, delta) = find(&h, "Add delta");
    assert!(charlie < delta, "oldest first, like git's todo list");
    find(&h, "pick   ");
    find(&h, "2 commits → 2 · p pick · s squash · f fixup · d drop");
}

#[test]
fn reordering_changes_the_history_but_not_the_content() {
    let repo = linear();
    let before = tree(&repo);
    let mut h = open(&repo);
    open_panel(&mut h);
    // Charlie (selected) moves after delta.
    press(&mut h, KeyCode::Char('J'));
    assert_eq!(panel_subjects(&h), ["Add delta", "Add charlie"]);
    start_and_confirm(&mut h);
    assert!(h.state.rebase.is_none(), "panel closed");
    assert_eq!(message(&h), "✓ Interactive Rebase");
    assert_eq!(
        subjects(&repo),
        "Add charlie\nAdd delta\nAdd bravo\nAdd alpha"
    );
    assert_eq!(tree(&repo), before, "same content");
    assert_eq!(repo.git(&["symbolic-ref", "--short", "HEAD"]), "main");
}

#[test]
fn alt_arrows_move_the_selected_commit_too() {
    let repo = linear();
    let mut h = open(&repo);
    open_panel(&mut h);
    key_with(&mut h, KeyCode::Down, KeyModifiers::ALT);
    assert_eq!(panel_subjects(&h), ["Add delta", "Add charlie"]);
    key_with(&mut h, KeyCode::Up, KeyModifiers::ALT);
    assert_eq!(panel_subjects(&h), ["Add charlie", "Add delta"]);
    // Plain arrows only select.
    press(&mut h, KeyCode::Down);
    assert_eq!(h.state.rebase.as_ref().map(|v| v.selected), Some(1));
    assert_eq!(panel_subjects(&h), ["Add charlie", "Add delta"]);
}

#[test]
fn squash_melds_a_commit_and_keeps_both_messages() {
    let repo = linear();
    let before = tree(&repo);
    let mut h = open(&repo);
    open_panel(&mut h);
    press(&mut h, KeyCode::Char('j'));
    press(&mut h, KeyCode::Char('s'));
    find(&h, "2 commits → 1");
    start_and_confirm(&mut h);
    assert_eq!(subjects(&repo), "Add charlie\nAdd bravo\nAdd alpha");
    assert_eq!(
        repo.git(&["log", "-1", "--format=%B"]),
        "Add charlie\n\nAdd delta"
    );
    assert_eq!(tree(&repo), before, "both changes kept");
}

#[test]
fn fixup_melds_a_commit_and_keeps_only_the_first_message() {
    let repo = linear();
    let before = tree(&repo);
    let mut h = open(&repo);
    open_panel(&mut h);
    press(&mut h, KeyCode::Char('j'));
    press(&mut h, KeyCode::Char('f'));
    start_and_confirm(&mut h);
    assert_eq!(subjects(&repo), "Add charlie\nAdd bravo\nAdd alpha");
    assert_eq!(repo.git(&["log", "-1", "--format=%B"]), "Add charlie");
    assert_eq!(tree(&repo), before, "both changes kept");
}

#[test]
fn drop_removes_a_commit_and_its_change() {
    let repo = linear();
    let mut h = open(&repo);
    open_panel(&mut h);
    press(&mut h, KeyCode::Char('d'));
    start_and_confirm(&mut h);
    assert_eq!(subjects(&repo), "Add delta\nAdd bravo\nAdd alpha");
    assert!(!repo.path().join("charlie.txt").exists(), "change gone");
    assert!(repo.path().join("delta.txt").exists());
}

#[test]
fn space_cycles_the_action() {
    let repo = linear();
    let mut h = open(&repo);
    open_panel(&mut h);
    press(&mut h, KeyCode::Char('j'));
    let action = |h: &Harness| h.state.rebase.as_ref().map(|v| v.rebase.items[1].action);
    for expected in [
        TodoAction::Squash,
        TodoAction::Fixup,
        TodoAction::Drop,
        TodoAction::Pick,
    ] {
        press(&mut h, KeyCode::Char(' '));
        assert_eq!(action(&h), Some(expected));
    }
}

#[test]
fn the_first_commit_cannot_be_squashed() {
    let repo = linear();
    let mut h = open(&repo);
    open_panel(&mut h);
    press(&mut h, KeyCode::Char('s'));
    assert_eq!(
        message(&h),
        "There is no kept commit before this one to squash it into."
    );
    let first = h.state.rebase.as_ref().map(|v| v.rebase.items[0].action);
    assert_eq!(first, Some(TodoAction::Pick));
}

#[test]
fn starting_without_a_change_is_refused() {
    let repo = linear();
    let before = data_hash(&repo);
    let mut h = open(&repo);
    open_panel(&mut h);
    press(&mut h, KeyCode::Enter);
    assert!(h.state.dialog.is_none());
    assert_eq!(
        message(&h),
        "Nothing to change: every commit is picked, in its original order."
    );
    assert_eq!(data_hash(&repo), before);
}

#[test]
fn esc_cancels_with_nothing_changed() {
    let repo = linear();
    let before = data_hash(&repo);
    let mut h = open(&repo);
    open_panel(&mut h);
    press(&mut h, KeyCode::Char('J'));
    press(&mut h, KeyCode::Char('d'));
    press(&mut h, KeyCode::Esc);
    assert!(h.state.rebase.is_none());
    assert_eq!(
        message(&h),
        "Interactive rebase cancelled: nothing changed."
    );
    assert_eq!(data_hash(&repo), before);
    // The graph has the keys back (bravo was selected for its menu).
    press(&mut h, KeyCode::Char('j'));
    assert_eq!(h.selected_subject(), Some("Add alpha"));
}

#[test]
fn cancelling_the_confirmation_keeps_the_panel_and_changes_nothing() {
    let repo = linear();
    let before = data_hash(&repo);
    let mut h = open(&repo);
    open_panel(&mut h);
    press(&mut h, KeyCode::Char('d'));
    press(&mut h, KeyCode::Enter);
    press(&mut h, KeyCode::Esc);
    assert!(h.state.dialog.is_none());
    let first = h.state.rebase.as_ref().map(|v| v.rebase.items[0].action);
    assert_eq!(first, Some(TodoAction::Drop), "edits kept");
    assert_eq!(data_hash(&repo), before);
}

#[test]
fn a_dirty_tree_is_refused_when_opening_and_when_starting() {
    let repo = linear();
    repo.write_file("alpha.txt", "edited\n");
    let mut h = open(&repo);
    choose(&mut h, "Add bravo", "Interactive Rebase from Here...");
    assert!(h.state.rebase.is_none());
    assert_eq!(message(&h), "Commit or stash your changes first.");

    // Clean when it opens, dirty by the time it starts.
    repo.git(&["checkout", "--", "alpha.txt"]);
    let commands = h.state.update(Event::Refresh);
    h.settle(commands);
    h.draw(W, H);
    open_panel(&mut h);
    press(&mut h, KeyCode::Char('d'));
    repo.write_file("alpha.txt", "edited again\n");
    let before = data_hash(&repo);
    let commands = h.state.update(Event::Refresh);
    h.settle(commands);
    press(&mut h, KeyCode::Enter);
    assert!(h.state.dialog.is_none(), "no confirmation");
    assert_eq!(message(&h), "Commit or stash your changes first.");
    assert_eq!(data_hash(&repo), before, "nothing changed");
    assert_eq!(
        subjects(&repo),
        "Add delta\nAdd charlie\nAdd bravo\nAdd alpha"
    );
}

#[test]
fn merge_commits_in_the_range_disable_the_entry() {
    let repo = FixtureRepo::sample();
    let before = data_hash(&repo);
    let mut h = open(&repo);
    // "Add main" is on main before the merge of develop.
    choose(&mut h, "Add main", "Interactive Rebase from Here...");
    assert!(h.state.rebase.is_none());
    assert_eq!(
        message(&h),
        "There are merge commits between this commit and HEAD."
    );
    assert_eq!(data_hash(&repo), before);
}

#[test]
fn a_conflicting_reorder_shows_the_banner_and_abort_restores_the_branch() {
    let repo = dependent();
    let head = repo.git(&["rev-parse", "HEAD"]);
    let mut h = open(&repo);
    open_panel(&mut h);
    press(&mut h, KeyCode::Char('J'));
    start_and_confirm(&mut h);
    assert_eq!(h.state.operation, Some(Operation::Rebase));
    assert!(
        message(&h).starts_with("Interactive Rebase stopped on conflicts:"),
        "{}",
        message(&h)
    );
    find(&h, "Rebase in progress · 1 conflicted file");

    press(&mut h, KeyCode::Char('A'));
    press(&mut h, KeyCode::Left);
    press(&mut h, KeyCode::Enter);
    assert_eq!(h.state.operation, None);
    assert_eq!(repo.git(&["rev-parse", "HEAD"]), head, "back where it was");
    assert_eq!(repo.git(&["symbolic-ref", "--short", "HEAD"]), "main");
    assert_eq!(repo.git(&["status", "--porcelain"]), "");
}

#[test]
fn a_commit_made_after_the_panel_opened_is_never_dropped_silently() {
    let repo = linear();
    let mut h = open(&repo);
    open_panel(&mut h);
    press(&mut h, KeyCode::Char('J'));
    // Someone commits outside the app while the panel is open.
    let echo = repo.commit_file("echo.txt", "echo\n", "Add echo");
    start_and_confirm(&mut h);
    // Git stops before replaying anything (rebase.missingCommitsCheck).
    assert_eq!(
        message(&h),
        "The branch changed after the rebase panel opened, so nothing was rewritten. \
         Abort the rebase (A) and open the panel again."
    );
    assert_eq!(h.state.operation, Some(Operation::Rebase));
    find(&h, "Rebase in progress");
    press(&mut h, KeyCode::Char('A'));
    press(&mut h, KeyCode::Left);
    press(&mut h, KeyCode::Enter);
    assert_eq!(h.state.operation, None);
    assert_eq!(repo.git(&["rev-parse", "HEAD"]), echo);
    assert_eq!(
        subjects(&repo),
        "Add echo\nAdd delta\nAdd charlie\nAdd bravo\nAdd alpha"
    );
}
