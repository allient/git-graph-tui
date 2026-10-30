//! Jumping to a parent commit that isn't loaded yet: history is paged in
//! until it appears.

use ggt_model::Oid;
use ggt_testkit::FixtureRepo;
use ggt_ui::DetailsOutcome;
use pretty_assertions::assert_eq;

use crate::testing::Harness;

/// 30 commits on `main`, `Commit 1` (the root) to `Commit 30`.
fn long_history() -> FixtureRepo {
    let repo = FixtureRepo::new();
    for i in 1..=30 {
        repo.commit_file("log.txt", &format!("{i}\n"), &format!("Commit {i}"));
    }
    repo
}

fn root(repo: &FixtureRepo) -> Oid {
    Oid::parse(&repo.git(&["rev-list", "--max-parents=0", "HEAD"])).unwrap()
}

#[test]
fn jumping_to_an_unloaded_commit_pages_history_until_it_is_found() {
    let repo = long_history();
    let mut h = Harness::with_page_size(&repo, 3);
    let root = root(&repo);
    assert!(
        !h.state.commits.iter().any(|c| c.id == root),
        "not loaded yet"
    );

    let commands = h
        .state
        .on_details_outcome(DetailsOutcome::JumpTo(root.clone()));
    assert!(
        h.state
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("Loading older commits"),
        "{:?}",
        h.state.message
    );
    h.settle(commands);

    assert_eq!(h.selected_subject(), Some("Commit 1"));
    assert_eq!(h.state.details.as_ref().map(|d| &d.commit), Some(&root));
    assert!(
        h.state.details.as_ref().is_some_and(|d| d.view.is_some()),
        "details were read"
    );
    assert_eq!(h.state.message, None);
}

#[test]
fn a_loaded_commit_is_selected_at_once() {
    let repo = long_history();
    let mut h = Harness::with_page_size(&repo, 3);
    let second = h.state.commits[1].id.clone();
    let commands = h
        .state
        .on_details_outcome(DetailsOutcome::JumpTo(second.clone()));
    h.settle(commands);
    assert_eq!(h.selected_subject(), Some("Commit 29"));
    assert_eq!(h.state.details.as_ref().map(|d| &d.commit), Some(&second));
}

#[test]
fn a_commit_missing_from_the_whole_history_is_reported() {
    let repo = long_history();
    let mut h = Harness::with_page_size(&repo, 10);
    let selected = h.state.selected;
    let missing = Oid::parse(&"f".repeat(40)).unwrap();
    let commands = h.state.on_details_outcome(DetailsOutcome::JumpTo(missing));
    h.settle(commands);
    assert!(h.state.history_complete);
    assert_eq!(
        h.state.message.as_deref(),
        Some("ffffffff is not in the loaded history.")
    );
    assert_eq!(h.state.selected, selected, "selection unchanged");
}
