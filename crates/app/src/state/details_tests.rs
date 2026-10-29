//! The inline commit details panel and the diff view, against real
//! fixture repositories.

use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use pretty_assertions::assert_eq;
use tgg_testkit::FixtureRepo;

use super::*;
use crate::testing::Harness;

/// Wide enough for full hashes beside the Source Control pane.
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

fn click(h: &mut Harness, x: u16, y: u16) {
    let commands = h.state.update(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }));
    h.settle(commands);
    h.draw(W, H);
}

fn click_text(h: &mut Harness, text: &str) {
    let (x, y) = h
        .find_on_screen(text)
        .unwrap_or_else(|| panic!("{text:?} not on screen"));
    click(h, x, y);
}

/// Selects the commit with `subject` using the keyboard.
fn select(h: &mut Harness, subject: &str) {
    press(h, KeyCode::Char('g'));
    while h.selected_subject() != Some(subject) {
        let before = h.state.selected;
        press(h, KeyCode::Down);
        assert_ne!(h.state.selected, before, "{subject:?} not found");
    }
}

fn hash(repo: &FixtureRepo, subject: &str) -> String {
    repo.git(&["log", "--all", "--format=%H", &format!("--grep={subject}")])
}

#[test]
fn enter_opens_the_details_under_the_commit_and_esc_closes_them() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    select(&mut h, "Add lib");
    press(&mut h, KeyCode::Enter);

    let full = hash(&repo, "Add lib");
    assert!(h.find_on_screen(&full).is_some(), "commit hash");
    assert!(
        h.find_on_screen("Fixture Author <fixture@example.com>")
            .is_some()
    );
    assert!(h.find_on_screen("Changed files (1)").is_some());
    assert!(h.find_on_screen("lib.rs").is_some());
    // The panel opens right below the commit's row.
    let (_, row) = h.find_on_screen("Add lib").unwrap();
    let (_, panel) = h.find_on_screen("Commit:").unwrap();
    assert_eq!(panel, row + 1);

    press(&mut h, KeyCode::Esc);
    assert!(h.state.details.is_none());
    assert!(h.find_on_screen("Commit:").is_none());
    assert!(!h.state.should_quit, "Esc closes the panel, not the app");
}

#[test]
fn clicking_a_commit_toggles_its_details() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    click_text(&mut h, "Update docs");
    assert_eq!(h.selected_subject(), Some("Update docs"));
    assert!(h.find_on_screen("Changed files (1)").is_some());

    click_text(&mut h, "Add main");
    assert_eq!(h.selected_subject(), Some("Add main"));
    assert!(h.find_on_screen(&hash(&repo, "Add main")).is_some());
    assert!(h.find_on_screen(&hash(&repo, "Update docs")).is_none());

    click_text(&mut h, "Add main");
    assert!(h.state.details.is_none());
}

#[test]
fn opening_a_file_shows_its_diff_and_esc_returns_to_the_details() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    select(&mut h, "Add lib");
    press(&mut h, KeyCode::Enter);
    press(&mut h, KeyCode::Enter);

    assert!(h.state.diff.is_some());
    assert!(h.find_on_screen("+pub fn lib() {}").is_some());
    press(&mut h, KeyCode::Esc);
    assert!(h.state.diff.is_none());
    assert!(h.find_on_screen("Changed files (1)").is_some());
}

#[test]
fn clicking_a_file_opens_its_diff() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    select(&mut h, "Update docs");
    press(&mut h, KeyCode::Enter);
    click_text(&mut h, "README.md");
    assert!(h.find_on_screen("+Docs.").is_some());
}

#[test]
fn clicking_a_parent_jumps_to_it_and_shows_its_details() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    select(&mut h, "Add lib");
    press(&mut h, KeyCode::Enter);
    let parent = hash(&repo, "Add main");
    click_text(&mut h, &parent[..8]);

    assert_eq!(h.selected_subject(), Some("Add main"));
    assert!(h.find_on_screen(&parent).is_some());
}

#[test]
fn a_refresh_keeps_the_details_open() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    select(&mut h, "Add lib");
    press(&mut h, KeyCode::Enter);
    press(&mut h, KeyCode::Char('r'));
    assert!(h.find_on_screen("Changed files (1)").is_some());
    assert_eq!(h.selected_subject(), Some("Add lib"));
}

#[test]
fn enter_on_the_uncommitted_row_does_nothing() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    press(&mut h, KeyCode::Char('g'));
    press(&mut h, KeyCode::Enter);
    assert!(h.state.details.is_none());
}

#[test]
fn v_shows_the_selected_file_as_it_was_in_that_commit() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo);
    select(&mut h, "Add lib");
    press(&mut h, KeyCode::Enter);
    assert!(
        h.find_on_screen("enter diff  v view file  t difftool")
            .is_some(),
        "key hint"
    );
    press(&mut h, KeyCode::Char('v'));

    let short = &hash(&repo, "Add lib")[..7];
    assert!(h.state.diff.is_some());
    assert!(h.find_on_screen(&format!("src/lib.rs @ {short}")).is_some());
    // The content then, not the later "Return a value" version, numbered from 1.
    assert!(h.find_on_screen("1 │ pub fn lib() {}").is_some());
    assert!(h.find_on_screen("pub fn lib() -> u8").is_none());
    assert_eq!(h.state.message, None);

    press(&mut h, KeyCode::Esc);
    assert!(h.state.diff.is_none());
    assert!(h.find_on_screen("Changed files (1)").is_some());
}

#[test]
fn v_on_a_file_the_commit_deleted_shows_the_git_error() {
    let repo = FixtureRepo::new();
    repo.commit_file("gone.txt", "bye\n", "Add gone");
    repo.git(&["rm", "--quiet", "--", "gone.txt"]);
    repo.git(&["commit", "--quiet", "--message", "Remove gone"]);
    let mut h = open(&repo);
    select(&mut h, "Remove gone");
    press(&mut h, KeyCode::Enter);
    press(&mut h, KeyCode::Char('v'));

    assert!(h.state.diff.is_none());
    let message = h.state.message.clone().unwrap_or_default();
    assert!(message.contains("gone.txt"), "{message}");
    assert!(message.contains("git show"), "{message}");
    // The details stay open.
    assert!(h.find_on_screen("Changed files (1)").is_some());
}

// ---- issue links (S20) ----

/// The sample repo plus a commit whose body mentions `reference`, with its
/// details open.
fn details_mentioning(reference: &str, origin: Option<&str>) -> (FixtureRepo, Harness) {
    let repo = FixtureRepo::sample();
    if let Some(url) = origin {
        repo.git(&["remote", "set-url", "origin", url]);
    }
    repo.commit_file(
        "fix.txt",
        "fixed\n",
        &format!("Fix the login\n\nCloses {reference}."),
    );
    let mut h = open(&repo);
    select(&mut h, "Fix the login");
    press(&mut h, KeyCode::Enter);
    (repo, h)
}

#[test]
fn clicking_an_issue_reference_opens_it_on_the_github_origin() {
    let (_repo, mut h) = details_mentioning("#12", Some("git@github.com:allient/demo.git"));
    click_text(&mut h, "#12");
    assert_eq!(h.opened, ["https://github.com/allient/demo/issues/12"]);
    assert_eq!(
        h.state.message.as_deref(),
        Some("Opening https://github.com/allient/demo/issues/12")
    );
    assert!(h.state.details.is_some(), "the details stay open");
}

#[test]
fn a_configured_issue_url_wins_over_the_origin() {
    let repo = FixtureRepo::sample();
    repo.git(&[
        "remote",
        "set-url",
        "origin",
        "git@github.com:allient/demo.git",
    ]);
    repo.commit_file(
        "fix.txt",
        "fixed\n",
        "Fix the login\n\nPart of PROJ-7, not #3.",
    );
    let mut h = Harness::open(&repo);
    h.state.issues = crate::config::Issues {
        url: Some("https://jira.example.com/browse/PROJ-$1".into()),
        prefix: Some("PROJ-".into()),
    };
    h.draw(W, H);
    select(&mut h, "Fix the login");
    press(&mut h, KeyCode::Enter);
    click_text(&mut h, "PROJ-7");
    assert_eq!(h.opened, ["https://jira.example.com/browse/PROJ-7"]);
    click_text(&mut h, "#3");
    assert_eq!(h.opened.len(), 1, "#3 isn't a reference with this prefix");
}

#[test]
fn an_issue_without_a_page_explains_how_to_configure_one() {
    // The sample's origin is a local folder: no forge to link to.
    let (_repo, mut h) = details_mentioning("#12", None);
    click_text(&mut h, "#12");
    assert!(h.opened.is_empty());
    assert_eq!(
        h.state.message.as_deref(),
        Some(
            "Can't open #12: origin isn't on GitHub, GitLab or Bitbucket. Set [issues] url in the config file, for example url = \"https://jira.example.com/browse/PROJ-$1\"."
        )
    );
}

#[test]
fn an_issue_in_a_repository_without_origin_explains_too() {
    let (repo, mut h) = details_mentioning("#12", None);
    repo.git(&["remote", "remove", "origin"]);
    click_text(&mut h, "#12");
    assert!(h.opened.is_empty());
    let message = h.state.message.clone().unwrap_or_default();
    assert!(
        message.starts_with("Can't open #12: this repository has no origin remote."),
        "{message}"
    );
    assert!(message.contains("[issues] url"), "{message}");
}
