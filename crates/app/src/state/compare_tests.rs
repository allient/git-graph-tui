//! Comparing two commits, or a commit with the working tree, from the
//! commit menu, against real fixture repositories.

use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use pretty_assertions::assert_eq;
use tgg_testkit::FixtureRepo;

use super::*;
use crate::testing::Harness;

/// Wide enough for the Source Control pane and the comparison title.
const W: u16 = 150;
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

fn mouse_down(button: MouseButton, x: u16, y: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(button),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    })
}

/// Right-clicks the first place `on` appears and clicks the menu entry
/// `entry`; returns the commands without running them.
fn choose_unsettled(h: &mut Harness, on: &str, entry: &str) -> Vec<Command> {
    let (x, y) = h
        .find_on_screen(on)
        .unwrap_or_else(|| panic!("{on:?} not on screen"));
    let commands = h.state.update(mouse_down(MouseButton::Right, x + 1, y));
    h.settle(commands);
    h.draw(W, H);
    let (x, y) = h
        .find_on_screen(entry)
        .unwrap_or_else(|| panic!("menu entry {entry:?} not on screen"));
    h.state.update(mouse_down(MouseButton::Left, x, y))
}

fn choose(h: &mut Harness, on: &str, entry: &str) {
    let commands = choose_unsettled(h, on, entry);
    h.settle(commands);
    h.draw(W, H);
}

/// Some screen line shows `name` and, further right, `counts`.
fn has_row(h: &Harness, name: &str, counts: &str) -> bool {
    let area = h.screen.area;
    (area.top()..area.bottom()).any(|y| {
        let line: String = (area.left()..area.right())
            .map(|x| h.screen[(x, y)].symbol())
            .collect();
        line.find(name)
            .is_some_and(|at| line[at..].contains(counts))
    })
}

/// The first 7 characters of the hash of the commit with exactly `subject`.
fn short(repo: &FixtureRepo, subject: &str) -> String {
    repo.git(&["log", "--all", "--format=%H %s"])
        .lines()
        .find_map(|line| line.split_once(' ').filter(|(_, s)| *s == subject))
        .map_or_else(
            || panic!("no commit {subject:?}"),
            |(hash, _)| hash[..7].to_owned(),
        )
}

#[test]
fn select_for_compare_then_compare_with_selected_lists_the_changed_files() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    choose(&mut h, "Initial commit", "Select for Compare");
    let base = short(&repo, "Initial commit");
    assert_eq!(
        h.state.message.as_deref(),
        Some(
            format!(
                "Selected {base} for compare. Right-click another commit and choose \"Compare with Selected\"."
            )
            .as_str()
        )
    );

    choose(&mut h, "Add lib", "Compare with Selected");
    let other = short(&repo, "Add lib");
    let title = format!("Comparing {base}..{other} (2 changed files)");
    assert!(h.find_on_screen(&title).is_some(), "{title}");
    // The panel opens right below the commit chosen last.
    let (_, row) = h.find_on_screen("Add lib").unwrap();
    let (_, panel) = h.find_on_screen(&title).unwrap();
    assert_eq!(panel, row + 1);
    assert!(has_row(&h, "lib.rs", "(+1 -0)"));
    assert!(has_row(&h, "main.rs", "(+1 -0)"));

    // Enter opens the selected file's diff between the two commits.
    press(&mut h, KeyCode::Down);
    press(&mut h, KeyCode::Enter);
    assert!(h.state.diff.is_some());
    assert!(h.find_on_screen("+fn main() {}").is_some());
    press(&mut h, KeyCode::Esc);
    assert!(h.state.diff.is_none());
    assert!(h.find_on_screen(&title).is_some(), "back to the comparison");

    // Esc closes the comparison, not the app.
    press(&mut h, KeyCode::Esc);
    assert!(h.state.details.is_none());
    assert!(h.find_on_screen("Comparing").is_none());
    assert!(!h.state.should_quit);
}

#[test]
fn compare_with_working_tree_includes_uncommitted_changes() {
    let repo = FixtureRepo::sample();
    repo.write_file("src/main.rs", "fn main() { changed() }\n");
    let mut h = open(&repo);
    choose(&mut h, "Add lib", "Compare with Working Tree");

    let base = short(&repo, "Add lib");
    let title = format!("Comparing {base}..working tree");
    assert!(h.find_on_screen(&title).is_some(), "{title}");
    // README.md changed on main since; src/main.rs only in the working tree.
    assert!(has_row(&h, "README.md", "(+2 -0)"));
    assert!(has_row(&h, "main.rs", "(+1 -1)"));

    // The first file row is src/main.rs (folders come first).
    press(&mut h, KeyCode::Enter);
    assert!(h.find_on_screen("+fn main() { changed() }").is_some());
    assert!(h.find_on_screen("-fn main() {}").is_some());
}

#[test]
fn compare_with_selected_is_only_offered_for_another_commit() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    choose(&mut h, "Add lib", "Select for Compare");
    let (x, y) = h.find_on_screen("Add lib").unwrap();
    let commands = h.state.update(mouse_down(MouseButton::Right, x + 1, y));
    h.settle(commands);
    h.draw(W, H);
    assert!(h.find_on_screen("Select for Compare").is_some());
    // The trailing space matches the menu row, not the status message.
    assert!(h.find_on_screen("Compare with Selected ").is_none());
}

#[test]
fn a_replaced_comparison_ignores_its_late_results() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    choose(&mut h, "Initial commit", "Select for Compare");
    // The first comparison's read is still in flight...
    let late = choose_unsettled(&mut h, "Add lib", "Compare with Selected");
    assert!(
        late.iter()
            .any(|c| matches!(c, Command::Git(Request::CompareFiles(_)))),
        "{late:?}"
    );
    // ... when a second one replaces it.
    // HEAD: the working tree has no tracked changes.
    choose(
        &mut h,
        "Merge branch 'develop'",
        "Compare with Working Tree",
    );
    h.settle(late);
    h.draw(W, H);

    let base = short(&repo, "Merge branch 'develop'");
    assert!(
        h.find_on_screen(&format!("Comparing {base}..working tree (0 changed files)"))
            .is_some()
    );
    assert!(h.find_on_screen("lib.rs").is_none());
}

#[test]
fn commit_details_arriving_after_a_comparison_opened_are_ignored() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    // Open the details of "Add lib" but don't run the reads yet.
    let (x, y) = h.find_on_screen("Add lib").unwrap();
    let late = h.state.update(mouse_down(MouseButton::Left, x + 1, y));
    assert!(!late.is_empty());
    let compare = choose_unsettled(&mut h, "Add lib", "Compare with Working Tree");
    // The details reads finish first: they must not fill the comparison.
    h.settle(late);
    h.draw(W, H);
    assert!(h.state.details.as_ref().is_some_and(|d| d.view.is_none()));
    assert!(h.find_on_screen("Changed files (1)").is_none());
    assert!(h.find_on_screen("Loading the comparison…").is_some());

    h.settle(compare);
    h.draw(W, H);
    let base = short(&repo, "Add lib");
    assert!(
        h.find_on_screen(&format!("Comparing {base}..working tree"))
            .is_some()
    );
    assert!(h.find_on_screen("Changed files (1)").is_none());
}
