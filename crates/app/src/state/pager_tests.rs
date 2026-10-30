//! An external diff pager and `git difftool` (S12) from the details panel
//! and comparisons, against real fixture repositories.

use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ggt_testkit::FixtureRepo;
use pretty_assertions::assert_eq;

use super::*;
use crate::external::{self, External};
use crate::testing::Harness;

const W: u16 = 150;
const H: u16 = 30;

fn open(repo: &FixtureRepo, pager: Option<&str>) -> Harness {
    let mut h = Harness::open(repo);
    h.state.pager = pager.map(Into::into);
    h.draw(W, H);
    h
}

fn press(h: &mut Harness, code: KeyCode) {
    h.key(code);
    h.draw(W, H);
}

/// Selects the commit with `subject` and opens its details.
fn open_details(h: &mut Harness, subject: &str) {
    press(h, KeyCode::Char('g'));
    while h.selected_subject() != Some(subject) {
        let before = h.state.selected;
        press(h, KeyCode::Down);
        assert_ne!(h.state.selected, before, "{subject:?} not found");
    }
    press(h, KeyCode::Enter);
}

fn hash(repo: &FixtureRepo, subject: &str) -> String {
    repo.git(&[
        "log",
        "--all",
        "--format=%H",
        &format!("--grep=^{subject}$"),
    ])
}

fn mouse_down(button: MouseButton, x: u16, y: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(button),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    })
}

/// Right-clicks `on` and clicks the menu entry `entry`.
fn choose(h: &mut Harness, on: &str, entry: &str) {
    let (x, y) = h.find_on_screen(on).unwrap();
    let commands = h.state.update(mouse_down(MouseButton::Right, x + 1, y));
    h.settle(commands);
    h.draw(W, H);
    let (x, y) = h.find_on_screen(entry).unwrap();
    let commands = h.state.update(mouse_down(MouseButton::Left, x, y));
    h.settle(commands);
    h.draw(W, H);
}

fn repo_path(h: &Harness) -> String {
    h.state.repo.to_string_lossy().into_owned()
}

fn sh(line: String) -> Vec<String> {
    vec!["-c".to_owned(), line]
}

#[test]
fn without_a_pager_enter_opens_the_built_in_diff() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo, None);
    open_details(&mut h, "Add lib");
    press(&mut h, KeyCode::Enter);
    assert!(h.state.diff.is_some());
    assert!(h.find_on_screen("+pub fn lib() {}").is_some());
    assert_eq!(h.external, Vec::new());
}

#[test]
fn with_a_pager_enter_pipes_the_diff_into_it_instead() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo, Some("delta --side-by-side"));
    open_details(&mut h, "Add lib");
    press(&mut h, KeyCode::Enter);

    assert!(h.state.diff.is_none(), "no built-in diff panel");
    let id = hash(&repo, "Add lib");
    let line = format!(
        "git --no-pager -C '{}' 'show' '--color=always' '--diff-merges=first-parent' \
         '--find-renames' '--format=' '--no-ext-diff' '{id}' '--' ':(literal)src/lib.rs' \
         | delta --side-by-side",
        repo_path(&h)
    );
    assert_eq!(
        h.external,
        vec![External {
            label: "delta --side-by-side".into(),
            program: "sh".into(),
            args: sh(line),
            refresh: false,
        }]
    );
    assert!(h.state.details.is_some(), "the details stay open");
}

#[test]
fn paths_with_spaces_and_quotes_are_quoted_for_the_shell() {
    let repo = FixtureRepo::new();
    repo.commit_file("my file's.rs", "x\n", "Odd name");
    let mut h = open(&repo, Some("delta"));
    open_details(&mut h, "Odd name");
    press(&mut h, KeyCode::Enter);
    let args = &h.external[0].args;
    assert!(
        args[1].ends_with(r"'--' ':(literal)my file'\''s.rs' | delta"),
        "{args:?}"
    );
}

#[test]
fn the_piped_diff_reaches_the_pager() {
    let repo = FixtureRepo::sample();
    let out = tempfile::tempdir().unwrap();
    let file = out.path().join("seen.txt");
    let pager = format!("cat > {}", external::quote(&file.to_string_lossy()));
    let mut h = open(&repo, Some(&pager));
    open_details(&mut h, "Add lib");
    press(&mut h, KeyCode::Enter);

    assert_eq!(external::run(&h.external[0]), None);
    let seen = std::fs::read_to_string(&file).unwrap();
    // Colored (escape sequences), so the pager can keep the colors.
    assert!(seen.contains("\u{1b}["), "{seen:?}");
    assert!(seen.contains("+++ b/src/lib.rs"), "{seen:?}");
    assert!(seen.contains("pub fn lib() {}"), "{seen:?}");
}

#[test]
fn a_comparison_pipes_its_diff_into_the_pager() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo, Some("delta"));
    choose(&mut h, "Initial commit", "Select for Compare");
    choose(&mut h, "Add lib", "Compare with Selected");
    press(&mut h, KeyCode::Enter);

    assert!(h.state.diff.is_none());
    let (base, other) = (hash(&repo, "Initial commit"), hash(&repo, "Add lib"));
    let line = format!(
        "git --no-pager -C '{}' 'diff' '--color=always' '--find-renames' '--no-ext-diff' \
         '{base}' '{other}' '--' ':(literal)src/lib.rs' | delta",
        repo_path(&h)
    );
    assert_eq!(h.external[0].args, sh(line));
}

#[test]
fn t_runs_difftool_against_the_first_parent() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo, None);
    open_details(&mut h, "Add lib");
    press(&mut h, KeyCode::Char('t'));

    let (id, parent) = (hash(&repo, "Add lib"), hash(&repo, "Add main"));
    assert_eq!(
        h.external,
        vec![External {
            label: "git difftool".into(),
            program: "git".into(),
            args: vec![
                "-C".into(),
                repo_path(&h),
                "difftool".into(),
                "--no-prompt".into(),
                parent,
                id,
                "--".into(),
                ":(literal)src/lib.rs".into(),
            ],
            refresh: false,
        }]
    );
    assert!(h.state.diff.is_none());
}

#[test]
fn t_on_a_root_commit_compares_with_the_empty_tree() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo, None);
    open_details(&mut h, "Initial commit");
    press(&mut h, KeyCode::Char('t'));
    let args = &h.external[0].args;
    assert_eq!(args[4], "4b825dc642cb6eb9a060e54bf8d69288fbee4904");
    assert_eq!(args[5], hash(&repo, "Initial commit"));
}

#[test]
fn t_in_a_comparison_with_the_working_tree_names_one_revision() {
    let repo = FixtureRepo::sample();
    repo.write_file("src/main.rs", "fn main() { changed() }\n");
    let mut h = open(&repo, None);
    choose(&mut h, "Add main", "Compare with Working Tree");
    // The first file is selected when the comparison opens.
    let file = h
        .state
        .details
        .as_ref()
        .and_then(|d| d.view.as_ref())
        .and_then(|v| v.selected_file())
        .map(|f| f.path.clone())
        .unwrap();
    press(&mut h, KeyCode::Char('t'));
    assert_eq!(
        h.external[0].args[2..],
        [
            "difftool".to_owned(),
            "--no-prompt".to_owned(),
            hash(&repo, "Add main"),
            "--".to_owned(),
            format!(":(literal){file}"),
        ]
    );
}

#[test]
fn t_does_nothing_while_no_file_is_selected() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo, None);
    press(&mut h, KeyCode::Char('t'));
    assert_eq!(h.external, Vec::new(), "no details open");
}
