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

/// Clicks "..." in the Source Control pane's header (the toolbar has none
/// while the pane shows).
fn open_repo_menu(h: &mut Harness) {
    let (x, y) = find(h, "✓ ↻ …");
    mouse(h, MouseEventKind::Down(MouseButton::Left), x + 4, y);
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
    open_repo_menu(&mut h);
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
    open_repo_menu(&mut h);
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
    open_repo_menu(&mut h);
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
    // Branch filter → remotes → fetch.
    for _ in 0..2 {
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

#[test]
fn the_repository_menu_button_moves_to_the_toolbar_on_narrow_terminals() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    assert!(h.find_on_screen("[...]").is_none(), "the pane has it");
    h.draw(80, H);
    click_text(&mut h, "[...]");
    assert!(h.state.menu.is_some());
}

/// Opens a submenu of the repository menu (its row is below "Fetch") and
/// clicks `entry` in it.
fn repo_submenu_entry(h: &mut Harness, submenu: &str, entry: &str) {
    open_repo_menu(h);
    // Only right of the menu's left edge: the pane lists "Staged Changes" too.
    let (left, below) = find(h, "Fetch");
    let (x, y) = find_in_menu(h, &format!("{submenu} "), left, below);
    mouse(h, MouseEventKind::Down(MouseButton::Left), x, y);
    let (x, y) = find_in_menu(h, entry, left, below);
    mouse(h, MouseEventKind::Down(MouseButton::Left), x, y);
}

/// Like [`find_below`], counting only cells from column `left` on.
fn find_in_menu(h: &Harness, text: &str, left: u16, min_y: u16) -> (u16, u16) {
    let area = h.screen.area;
    (min_y + 1..area.bottom())
        .find_map(|y| {
            let line: String = (left..area.right())
                .map(|x| h.screen[(x, y)].symbol())
                .collect();
            let byte = line.find(text)?;
            let x = left + u16::try_from(line[..byte].chars().count()).ok()?;
            Some((x, y))
        })
        .unwrap_or_else(|| panic!("{text:?} not in the menu below line {min_y}"))
}

fn head_subject(repo: &FixtureRepo) -> String {
    repo.git(&["log", "-1", "--format=%s"])
}

fn commit_count(repo: &FixtureRepo) -> String {
    repo.git(&["rev-list", "--count", "HEAD"])
}

/// Types `message` in the Source Control message box, then leaves the pane.
fn type_commit_message(h: &mut Harness, message: &str) {
    press(h, KeyCode::Char('c'));
    type_text(h, message);
    press(h, KeyCode::Esc);
}

#[test]
fn commit_all_from_the_menu_commits_every_change_with_the_pane_message() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    type_commit_message(&mut h, "Add notes");
    repo_submenu_entry(&mut h, "Commit", "Commit All");
    assert_eq!(head_subject(&repo), "Add notes");
    assert_eq!(status_of(&repo), "");
    assert_eq!(h.state.source_control.view.message(), "");
}

#[test]
fn commit_from_the_menu_needs_a_message_unless_it_amends() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    let before = commit_count(&repo);
    let subject = head_subject(&repo);
    repo_submenu_entry(&mut h, "Commit", "Commit All");
    assert_eq!(
        h.state.message.as_deref(),
        Some("Type a commit message first.")
    );
    assert_eq!(commit_count(&repo), before);

    // An empty message amends with the last commit's message.
    press(&mut h, KeyCode::Esc);
    repo_submenu_entry(&mut h, "Commit", "Commit All (Amend)");
    assert_eq!(commit_count(&repo), before);
    assert_eq!(head_subject(&repo), subject);
    assert_eq!(status_of(&repo), "");
}

#[test]
fn signed_off_commits_carry_the_trailer() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    type_commit_message(&mut h, "Add notes");
    repo_submenu_entry(&mut h, "Commit", "Commit All (Signed Off)");
    let body = repo.git(&["log", "-1", "--format=%B"]);
    assert!(body.contains("Signed-off-by: "), "{body}");
}

#[test]
fn undo_last_commit_keeps_its_changes_staged() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    let parent = repo.git(&["rev-parse", "HEAD~1"]);
    repo_submenu_entry(&mut h, "Commit", "Undo Last Commit");
    assert_eq!(repo.git(&["rev-parse", "HEAD"]), parent);
    let staged = repo.git(&["diff", "--cached", "--name-only"]);
    assert!(!staged.is_empty(), "the undone commit's files are staged");
}

#[test]
fn the_changes_submenu_stages_unstages_and_discards_everything() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    repo_submenu_entry(&mut h, "Changes", "Stage All Changes");
    assert_eq!(status_of(&repo), "A  notes.txt");
    repo_submenu_entry(&mut h, "Changes", "Unstage All Changes");
    assert_eq!(status_of(&repo), "?? notes.txt");
    repo_submenu_entry(&mut h, "Changes", "Discard All Changes");
    // Destructive: focus starts on Cancel, so move to Discard first.
    assert!(matches!(h.state.dialog, Some(Dialog::Confirm { .. })));
    press(&mut h, KeyCode::Left);
    press(&mut h, KeyCode::Enter);
    assert_eq!(status_of(&repo), "");
}

fn status_of(repo: &FixtureRepo) -> String {
    repo.git(&["status", "--porcelain", "--untracked-files=all"])
}

/// Types `name` in the open ref picker and picks the first match.
fn pick(h: &mut Harness, name: &str) {
    assert!(h.state.picker.is_some(), "no picker open");
    type_text(h, name);
    press(h, KeyCode::Enter);
}

/// Presses Enter on every input dialog and the confirm button of every
/// confirmation, until no dialog is left.
fn accept_dialogs(h: &mut Harness) {
    for _ in 0..4 {
        match h.state.dialog {
            Some(Dialog::Input { .. }) => press(h, KeyCode::Enter),
            Some(Dialog::Confirm { .. }) => {
                // Destructive confirmations start on Cancel.
                press(h, KeyCode::Left);
                press(h, KeyCode::Enter);
            }
            _ => return,
        }
    }
}

#[test]
fn branch_submenu_merges_a_picked_branch_into_the_current_one() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    repo_submenu_entry(&mut h, "Branch", "Merge...");
    pick(&mut h, "feature/login");
    accept_dialogs(&mut h);
    // `git` fails the test unless feature/login is now in HEAD's history.
    repo.git(&["merge-base", "--is-ancestor", "feature/login", "HEAD"]);
    assert_eq!(current_branch(&repo), "main");
}

#[test]
fn create_branch_from_a_picked_ref_checks_it_out() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    repo_submenu_entry(&mut h, "Branch", "Create Branch From...");
    pick(&mut h, "develop");
    assert!(matches!(h.state.dialog, Some(Dialog::Input { .. })));
    type_text(&mut h, "from-develop");
    accept_dialogs(&mut h);
    assert_eq!(current_branch(&repo), "from-develop");
    assert_eq!(
        repo.git(&["rev-parse", "HEAD"]),
        repo.git(&["rev-parse", "develop"])
    );
}

#[test]
fn delete_remote_branch_asks_then_deletes_it_on_the_remote() {
    let repo = FixtureRepo::sample();
    repo.git(&["push", "--quiet", "origin", "develop:refs/heads/gone"]);
    repo.git(&["fetch", "--quiet", "origin"]);
    let mut h = open(&repo);
    repo_submenu_entry(&mut h, "Branch", "Delete Remote Branch...");
    pick(&mut h, "origin/gone");
    assert!(matches!(h.state.dialog, Some(Dialog::Confirm { .. })));
    accept_dialogs(&mut h);
    assert_eq!(repo.git(&["ls-remote", "origin", "refs/heads/gone"]), "");
}

fn stash_list(repo: &FixtureRepo) -> String {
    repo.git(&["stash", "list", "--format=%gd %s"])
}

#[test]
fn stash_submenu_stashes_then_applies_views_and_drops_a_picked_stash() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    // The sample's only change is the untracked notes.txt.
    repo_submenu_entry(&mut h, "Stash", "Stash (Include Untracked)");
    type_text(&mut h, "notes");
    accept_dialogs(&mut h);
    assert!(
        stash_list(&repo).starts_with("stash@{0} On main: notes"),
        "{}",
        stash_list(&repo)
    );
    assert_eq!(status_of(&repo), "");

    repo_submenu_entry(&mut h, "Stash", "View Stash...");
    pick(&mut h, "stash@{0}");
    assert!(h.state.diff.is_some(), "the stash's diff is shown");
    press(&mut h, KeyCode::Esc);

    repo_submenu_entry(&mut h, "Stash", "Apply Stash...");
    pick(&mut h, "stash@{0}");
    accept_dialogs(&mut h);
    assert_eq!(status_of(&repo), "?? notes.txt");
    assert!(!stash_list(&repo).is_empty(), "apply keeps the stash");

    repo_submenu_entry(&mut h, "Stash", "Drop All Stashes...");
    accept_dialogs(&mut h);
    assert_eq!(stash_list(&repo), "");
}

#[test]
fn tags_submenu_deletes_a_picked_tag_here_or_on_the_remote() {
    let repo = FixtureRepo::sample();
    repo.git(&["push", "--quiet", "origin", "v0.1.0"]);
    let mut h = open(&repo);
    repo_submenu_entry(&mut h, "Tags", "Delete Remote Tag...");
    pick(&mut h, "v0.1.0");
    accept_dialogs(&mut h);
    assert_eq!(repo.git(&["ls-remote", "--tags", "origin"]), "");
    assert_eq!(
        repo.git(&["tag", "--list"]),
        "v0.1.0",
        "the local tag stays"
    );

    repo_submenu_entry(&mut h, "Tags", "Delete Tag...");
    pick(&mut h, "v0.1.0");
    accept_dialogs(&mut h);
    assert_eq!(repo.git(&["tag", "--list"]), "");
}
