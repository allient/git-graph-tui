//! `worktree list` parser run against output from a real git, on fixture repos.

#![expect(clippy::unwrap_used, reason = "test helpers")]

use std::path::Path;

use pretty_assertions::assert_eq;
use tgg_git::GitCommand;
use tgg_git::worktree::{self, Worktree};
use tgg_testkit::FixtureRepo;

fn worktrees(repo: &FixtureRepo) -> Vec<Worktree> {
    let full = GitCommand::read(repo.path(), worktree::args()).argv();
    let full: Vec<&str> = full.iter().map(String::as_str).collect();
    worktree::parse(&repo.git_raw(&full)).unwrap()
}

/// The folder name of a worktree path.
fn name(path: &str) -> &str {
    Path::new(path).file_name().unwrap().to_str().unwrap()
}

#[test]
fn lists_branch_detached_locked_and_prunable_worktrees() {
    let repo = FixtureRepo::sample();
    // Worktree folders go in a second throwaway directory, deleted with it.
    let scratch = FixtureRepo::new();
    let at = |name: &str| scratch.path().join(name).to_str().unwrap().to_owned();
    repo.git(&["worktree", "add", "--quiet", &at("dev wt"), "develop"]);
    repo.git(&["worktree", "add", "--quiet", "--detach", &at("usb"), "main"]);
    repo.git(&["worktree", "lock", "--reason", "on a usb stick", &at("usb")]);
    repo.git(&["worktree", "add", "--quiet", "-b", "gone", &at("gone")]);
    std::fs::remove_dir_all(at("gone")).unwrap();

    let list = worktrees(&repo);
    let main = repo.git(&["rev-parse", "main"]);
    let develop = repo.git(&["rev-parse", "develop"]);
    let summary: Vec<_> = list
        .iter()
        .map(|w| {
            (
                name(&w.path),
                w.head.as_ref().map(ToString::to_string),
                w.branch.as_deref(),
                w.detached,
                w.is_main,
            )
        })
        .collect();
    let root = name(repo.path().to_str().unwrap());
    assert_eq!(
        summary,
        [
            (root, Some(main.clone()), Some("main"), false, true),
            ("dev wt", Some(develop), Some("develop"), false, false),
            // Git sorts the linked worktrees by path.
            ("gone", Some(main.clone()), Some("gone"), false, false),
            ("usb", Some(main), None, true, false),
        ]
    );
    assert!(list[2].prunable.is_some(), "{:?}", list[2]);
    assert_eq!(list[3].locked.as_deref(), Some("on a usb stick"));
    assert!(list.iter().all(|w| !w.bare));
}

#[test]
fn a_bare_repository_lists_itself_as_bare() {
    let scratch = FixtureRepo::new();
    let bare = scratch.path().join("bare.git");
    scratch.git(&["init", "--quiet", "--bare", bare.to_str().unwrap()]);
    let full = GitCommand::read(&bare, worktree::args()).argv();
    let full: Vec<&str> = full.iter().map(String::as_str).collect();
    let list = worktree::parse(&scratch.git_raw(&full)).unwrap();
    assert_eq!(list.len(), 1);
    assert!(list[0].bare && list[0].is_main, "{list:?}");
    assert_eq!(list[0].head, None);
}
