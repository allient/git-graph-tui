//! `in_progress` against operations a real git stopped on a conflict.

#![expect(clippy::expect_used, reason = "test helpers")]

use std::process::Command;

use pretty_assertions::assert_eq;
use tgg_git::in_progress::{self, Operation};
use tgg_testkit::FixtureRepo;

/// Runs a git command that is expected to stop on a conflict (and so fail).
fn git_conflicting(repo: &FixtureRepo, args: &[&str]) {
    let output = Command::new("git")
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

/// `main` and `topic` both change the first line of `file.txt`.
fn diverged() -> FixtureRepo {
    let repo = FixtureRepo::new();
    repo.commit_file("file.txt", "base\n", "Base");
    repo.branch("topic");
    repo.commit_file("file.txt", "topic\n", "Topic edit");
    repo.checkout("main");
    repo.commit_file("file.txt", "main\n", "Main edit");
    repo
}

fn detect(repo: &FixtureRepo) -> Option<Operation> {
    let args = in_progress::args();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = repo.git_raw(&args);
    in_progress::parse(&output, |path| repo.path().join(path).exists())
}

#[test]
fn a_clean_repository_has_nothing_in_progress() {
    assert_eq!(detect(&diverged()), None);
}

#[test]
fn a_conflicted_merge() {
    let repo = diverged();
    git_conflicting(&repo, &["merge", "topic"]);
    assert_eq!(detect(&repo), Some(Operation::Merge));
}

#[test]
fn a_conflicted_rebase() {
    let repo = diverged();
    git_conflicting(&repo, &["rebase", "topic"]);
    assert_eq!(detect(&repo), Some(Operation::Rebase));
}

#[test]
fn a_conflicted_cherry_pick() {
    let repo = diverged();
    git_conflicting(&repo, &["cherry-pick", "topic"]);
    assert_eq!(detect(&repo), Some(Operation::CherryPick));
}

#[test]
fn a_conflicted_revert() {
    let repo = FixtureRepo::new();
    repo.commit_file("file.txt", "one\n", "One");
    repo.commit_file("file.txt", "two\n", "Two");
    repo.commit_file("file.txt", "three\n", "Three");
    git_conflicting(&repo, &["revert", "--no-edit", "HEAD~1"]);
    assert_eq!(detect(&repo), Some(Operation::Revert));
}
