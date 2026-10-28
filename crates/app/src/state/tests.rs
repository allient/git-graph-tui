use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pretty_assertions::assert_eq;
use tgg_git::GitCommand;
use tgg_model::Oid;
use tgg_testkit::FixtureRepo;

use super::*;
use crate::git::JobResult;
use crate::testing::Harness;

#[test]
fn refresh_loads_commits_refs_status_and_layout() {
    let repo = FixtureRepo::sample();
    let h = Harness::open(&repo);
    assert_eq!(h.state.commits.len(), 7);
    assert_eq!(h.state.graph.rows().len(), 7);
    assert_eq!(h.state.refs.len(), 6);
    assert_eq!(h.state.status.changed_files(), 1);
    assert_eq!(
        h.state.head,
        Oid::parse(&repo.git(&["rev-parse", "HEAD"])).ok()
    );
    assert_eq!(h.state.selected, 0, "the uncommitted row is selected first");
    assert!(h.state.message.is_none(), "{:?}", h.state.message);
}

#[test]
fn keyboard_moves_the_selection_within_bounds() {
    let repo = FixtureRepo::sample();
    let mut h = Harness::open(&repo);
    h.key(KeyCode::Char('j'));
    assert_eq!(h.selected_subject(), Some("Add login"));
    h.key(KeyCode::Down);
    assert_eq!(h.selected_subject(), Some("Return a value"));
    h.key(KeyCode::Char('k'));
    h.key(KeyCode::Up);
    h.key(KeyCode::Up);
    assert_eq!(h.state.selected, 0, "stops at the top");
    h.key(KeyCode::Char('G'));
    assert_eq!(h.selected_subject(), Some("Initial commit"));
    h.key(KeyCode::Down);
    assert_eq!(
        h.selected_subject(),
        Some("Initial commit"),
        "stops at the bottom"
    );
    h.key(KeyCode::Char('g'));
    assert_eq!(h.state.selected, 0);
}

#[test]
fn page_keys_move_by_the_viewport_height() {
    let repo = FixtureRepo::sample();
    let mut h = Harness::open(&repo);
    h.key(KeyCode::PageDown);
    assert_eq!(h.state.selected, 5);
    h.key(KeyCode::PageDown);
    assert_eq!(h.state.selected, 7, "clamped to the last row");
    h.key(KeyCode::PageUp);
    assert_eq!(h.state.selected, 2);
}

#[test]
fn the_viewport_scrolls_to_keep_the_selection_visible() {
    let repo = FixtureRepo::sample();
    let mut h = Harness::open(&repo);
    h.key(KeyCode::Char('G'));
    assert_eq!(
        h.state.offset, 3,
        "last row (7) is at the bottom of a 5-row viewport"
    );
    h.key(KeyCode::Char('g'));
    assert_eq!(h.state.offset, 0);
}

#[test]
fn jump_to_head_selects_the_head_commit() {
    let repo = FixtureRepo::sample();
    let mut h = Harness::open(&repo);
    h.key(KeyCode::Char('H'));
    assert_eq!(h.selected_subject(), Some("Merge branch 'develop'"));
}

#[test]
fn refresh_keeps_the_selection_on_the_same_commit() {
    let repo = FixtureRepo::sample();
    let mut h = Harness::open(&repo);
    h.key(KeyCode::Char('H'));
    repo.git(&["switch", "--quiet", "--discard-changes", "develop"]);
    repo.commit_file("new.txt", "1\n", "New on develop");
    h.key(KeyCode::Char('r'));
    assert_eq!(h.state.commits.len(), 8);
    assert_eq!(h.selected_subject(), Some("Merge branch 'develop'"));
}

#[test]
fn results_from_a_superseded_refresh_are_ignored() {
    let repo = FixtureRepo::sample();
    let mut h = Harness::open(&repo);
    // Start a refresh and let its git commands finish, but hold their results back.
    let stale = h.state.update(Event::Refresh);
    for Command::Git(request) in stale {
        let job = h.runner.submit(request.command(&h.state.repo));
        h.state.track(&job, request);
    }
    let stale_results: Vec<JobResult> = (0..3)
        .map(|_| h.results.recv_timeout(Duration::from_secs(10)).unwrap())
        .collect();

    // A second refresh supersedes the first and completes.
    let fresh = h.state.update(Event::Refresh);
    h.settle(fresh);
    let before = h.state.commits.clone();

    // The late results of the first refresh change nothing and request nothing.
    for result in stale_results {
        assert!(h.state.update(Event::Git(result)).is_empty());
    }
    assert_eq!(h.state.commits, before);
    assert!(!h.state.is_loading());
}

#[test]
fn more_commits_load_when_the_selection_nears_the_end() {
    let repo = FixtureRepo::sample();
    let mut h = Harness::with_page_size(&repo, 3);
    // A 5-row viewport with 3-commit pages loads two pages to fill the screen.
    assert_eq!(
        h.state.commits.len(),
        6,
        "pages load until the viewport is filled"
    );
    h.key(KeyCode::Char('G'));
    h.key(KeyCode::Char('G'));
    h.key(KeyCode::Char('G'));
    assert_eq!(h.state.commits.len(), 7, "pages load until history ends");
    assert!(h.state.history_complete);
    assert_eq!(h.selected_subject(), Some("Initial commit"));
    assert_eq!(h.state.graph.rows().len(), 7);
}

#[test]
fn empty_repository_has_no_commits_and_no_errors() {
    let repo = FixtureRepo::new();
    let h = Harness::open(&repo);
    assert!(h.state.commits.is_empty());
    assert_eq!(h.state.head, None);
    assert!(h.state.message.is_none(), "{:?}", h.state.message);
}

#[test]
fn git_failures_become_a_status_message() {
    let repo = FixtureRepo::sample();
    let mut h = Harness::open(&repo);
    let job = h
        .runner
        .submit(GitCommand::read(repo.path(), ["status", "--no-such-flag"]));
    h.state.track(&job, Request::Status);
    let result = h.results.recv_timeout(Duration::from_secs(10)).unwrap();
    h.state.update(Event::Git(result));
    assert!(
        h.state
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("no-such-flag"),
        "{:?}",
        h.state.message
    );
}

#[test]
fn quit_keys() {
    for code in [KeyCode::Char('q'), KeyCode::Esc] {
        let mut state = AppState::new(".".into());
        state.update(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
        assert!(state.should_quit, "{code:?}");
    }
    let mut state = AppState::new(".".into());
    state.update(Event::Key(KeyEvent::new(
        KeyCode::Char('c'),
        KeyModifiers::CONTROL,
    )));
    assert!(state.should_quit, "ctrl+c");
}

/// Replays the startup race where the log arrives before the status: the top
/// row was a commit, then the uncommitted row appears above it.
#[test]
fn status_arriving_after_the_log_keeps_the_top_row_selected() {
    let repo = FixtureRepo::sample();
    let mut h = Harness::open(&repo);
    let dirty = h.state.status.clone();
    h.state.status = WorkingTreeStatus::default();
    h.state.selected = 0;
    h.state.offset = 0;

    h.state.apply_status(dirty);
    assert_eq!(
        h.state.selected, 0,
        "the new top row (uncommitted) stays selected"
    );
}

#[test]
fn status_changes_keep_a_selected_commit() {
    let repo = FixtureRepo::sample();
    let mut h = Harness::open(&repo);
    h.key(KeyCode::Char('H'));
    let selected = h.selected_subject().map(str::to_owned);

    h.state.apply_status(WorkingTreeStatus::default());
    assert_eq!(h.selected_subject().map(str::to_owned), selected);
}
