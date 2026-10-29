//! Runs the tag, stash and uncommitted-changes plans against fixture repos.

#![expect(
    clippy::expect_used,
    reason = "test helpers panic so a broken fixture fails loudly"
)]

use std::process::Command;

use pretty_assertions::assert_eq;
use tgg_actions::{
    Action, InputId, InputValue, Plan, ResetMode, StashAction, TagAction, WorkingTreeAction,
};
use tgg_git::{Failure, GitCommand};
use tgg_model::{Oid, Stash};
use tgg_testkit::FixtureRepo;

fn run(repo: &FixtureRepo, action: &Action) -> Result<(), Failure> {
    let Plan::Git(commands) = action.plan(repo.path()) else {
        panic!("{action:?} doesn't run git");
    };
    commands.iter().try_for_each(exec)
}

fn exec(command: &GitCommand) -> Result<(), Failure> {
    let output = Command::new("git")
        .args(command.argv())
        .envs(command.env())
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("start git");
    if output.status.success() {
        Ok(())
    } else {
        Err(Failure::classify(&String::from_utf8_lossy(&output.stderr)))
    }
}

fn ok(repo: &FixtureRepo, action: &Action) {
    if let Err(failure) = run(repo, action) {
        panic!("{action:?} failed: {}", failure.message());
    }
}

fn status(repo: &FixtureRepo) -> String {
    repo.git(&["status", "--porcelain", "--untracked-files=all"])
}

fn remote_tags(repo: &FixtureRepo) -> String {
    repo.git(&["ls-remote", "--tags", "origin"])
}

/// The sample repo's stash, `stash@{0}`.
fn sample_stash(repo: &FixtureRepo) -> Stash {
    Stash {
        id: Oid::parse(&repo.git(&["rev-parse", "stash@{0}"])).expect("stash exists"),
        index: 0,
        subject: "On main: WIP on docs".into(),
    }
}

// ---- tags ----

#[test]
fn push_a_tag_then_delete_it_locally_and_on_the_remote() {
    let repo = FixtureRepo::sample();
    let tag = |action| Action::Tag(action);
    ok(
        &repo,
        &tag(TagAction::Push {
            name: "v0.1.0".into(),
            remote: "origin".into(),
        }),
    );
    assert!(remote_tags(&repo).ends_with("refs/tags/v0.1.0"));

    let mut delete = tag(TagAction::Delete {
        name: "v0.1.0".into(),
        remote: Some("origin".into()),
        on_remote: false,
    });
    ok(&repo, &delete);
    assert_eq!(repo.git(&["tag", "--list"]), "");
    assert!(
        !remote_tags(&repo).is_empty(),
        "local delete keeps the remote tag"
    );

    repo.tag("v0.1.0");
    delete
        .set_input(InputId::DeleteOnRemote, InputValue::Bool(true))
        .unwrap();
    ok(&repo, &delete);
    assert_eq!(repo.git(&["tag", "--list"]), "");
    assert_eq!(remote_tags(&repo), "");
}

// ---- stashes ----

#[test]
fn apply_keeps_the_stash_and_pop_removes_it() {
    let repo = FixtureRepo::sample();
    let stash = sample_stash(&repo);
    let apply = Action::Stash(StashAction::Apply {
        stash: stash.clone(),
        reinstate_index: false,
    });
    ok(&repo, &apply);
    assert!(status(&repo).contains(" M README.md"));
    assert_eq!(repo.git(&["stash", "list"]).lines().count(), 1);

    repo.git(&["checkout", "--quiet", "--", "README.md"]);
    ok(
        &repo,
        &Action::Stash(StashAction::Pop {
            stash,
            reinstate_index: true,
        }),
    );
    assert!(status(&repo).contains(" M README.md"));
    assert_eq!(repo.git(&["stash", "list"]), "");
}

#[test]
fn drop_deletes_the_stash() {
    let repo = FixtureRepo::sample();
    ok(
        &repo,
        &Action::Stash(StashAction::Drop {
            stash: sample_stash(&repo),
        }),
    );
    assert_eq!(repo.git(&["stash", "list"]), "");
}

#[test]
fn create_branch_from_a_stash() {
    let repo = FixtureRepo::sample();
    let stash = sample_stash(&repo);
    let base = repo.git(&["rev-parse", "stash@{0}^1"]);
    ok(
        &repo,
        &Action::Stash(StashAction::CreateBranch {
            stash,
            name: "rescued".into(),
        }),
    );
    assert_eq!(repo.git(&["branch", "--show-current"]), "rescued");
    assert_eq!(repo.git(&["rev-parse", "HEAD"]), base);
    assert!(status(&repo).contains(" M README.md"));
    assert_eq!(repo.git(&["stash", "list"]), "", "the stash is dropped");
}

// ---- uncommitted changes ----

#[test]
fn stash_uncommitted_changes_with_untracked_files_and_a_message() {
    let repo = FixtureRepo::sample();
    repo.write_file("README.md", "changed\n");
    assert!(status(&repo).contains("?? notes.txt"));
    let mut stash = Action::WorkingTree(WorkingTreeAction::Stash {
        message: String::new(),
        include_untracked: true,
    });
    stash
        .set_input(InputId::Message, InputValue::Text("Before lunch".into()))
        .unwrap();
    ok(&repo, &stash);
    assert_eq!(
        status(&repo),
        "",
        "tracked and untracked changes are stashed"
    );
    assert_eq!(
        repo.git(&["stash", "list", "--max-count=1", "--format=%gs"]),
        "On main: Before lunch"
    );
}

#[test]
fn stash_without_untracked_files_leaves_them() {
    let repo = FixtureRepo::sample();
    repo.write_file("README.md", "changed\n");
    ok(
        &repo,
        &Action::WorkingTree(WorkingTreeAction::Stash {
            message: String::new(),
            include_untracked: false,
        }),
    );
    assert_eq!(status(&repo), "?? notes.txt");
}

#[test]
fn reset_mixed_unstages_and_hard_discards() {
    let repo = FixtureRepo::sample();
    repo.write_file("README.md", "changed\n");
    repo.git(&["add", "README.md"]);
    assert!(status(&repo).contains("M  README.md"));

    let reset = |mode| Action::WorkingTree(WorkingTreeAction::Reset { mode });
    ok(&repo, &reset(ResetMode::Mixed));
    assert!(
        status(&repo).contains(" M README.md"),
        "unstaged, still changed"
    );

    ok(&repo, &reset(ResetMode::Hard));
    assert_eq!(status(&repo), "?? notes.txt", "untracked files are kept");
}

#[test]
fn clean_deletes_untracked_files_and_directories_but_not_ignored_ones() {
    let repo = FixtureRepo::sample();
    repo.write_file(".gitignore", "*.log\n");
    repo.git(&["add", ".gitignore"]);
    repo.write_file("build/out.txt", "x\n");
    repo.write_file("debug.log", "x\n");
    ok(&repo, &Action::WorkingTree(WorkingTreeAction::Clean));
    assert_eq!(status(&repo), "A  .gitignore");
    assert!(!repo.path().join("notes.txt").exists());
    assert!(!repo.path().join("build").exists());
    assert!(repo.path().join("debug.log").exists(), "ignored files stay");
}

#[test]
fn stage_unstage_and_commit_files() {
    let repo = FixtureRepo::sample();
    repo.write_file("README.md", "changed\n");
    repo.write_file("new dir/a b.txt", "x\n");
    let stage = |paths: &[&str]| {
        Action::WorkingTree(WorkingTreeAction::Stage {
            paths: paths.iter().map(|p| (*p).to_owned()).collect(),
        })
    };
    ok(&repo, &stage(&["README.md", "new dir/a b.txt"]));
    assert_eq!(
        status(&repo),
        "M  README.md\nA  \"new dir/a b.txt\"\n?? notes.txt"
    );

    ok(
        &repo,
        &Action::WorkingTree(WorkingTreeAction::Unstage {
            paths: vec!["README.md".into()],
        }),
    );
    assert!(
        status(&repo).starts_with(" M README.md"),
        "{}",
        status(&repo)
    );

    ok(
        &repo,
        &Action::WorkingTree(WorkingTreeAction::Commit {
            message: "Add a b".into(),
            all: false,
            amend: false,
            signoff: false,
        }),
    );
    assert_eq!(repo.git(&["log", "-1", "--format=%s"]), "Add a b");
    assert_eq!(status(&repo), " M README.md\n?? notes.txt");
}

#[test]
fn staging_a_deleted_file_and_unstaging_in_a_repo_without_commits() {
    let repo = FixtureRepo::sample();
    std::fs::remove_file(repo.path().join("README.md")).unwrap();
    ok(
        &repo,
        &Action::WorkingTree(WorkingTreeAction::Stage {
            paths: vec!["README.md".into()],
        }),
    );
    assert!(
        status(&repo).starts_with("D  README.md"),
        "{}",
        status(&repo)
    );

    let fresh = FixtureRepo::new();
    fresh.write_file("first.txt", "1\n");
    fresh.git(&["add", "first.txt"]);
    ok(
        &fresh,
        &Action::WorkingTree(WorkingTreeAction::Unstage {
            paths: vec!["first.txt".into()],
        }),
    );
    assert_eq!(status(&fresh), "?? first.txt");
}

#[test]
fn discard_restores_changed_files_and_deletes_untracked_ones() {
    let repo = FixtureRepo::sample();
    repo.write_file("README.md", "changed\n");
    ok(
        &repo,
        &Action::WorkingTree(WorkingTreeAction::Discard {
            paths: vec!["README.md".into()],
            untracked: vec!["notes.txt".into()],
        }),
    );
    assert_eq!(status(&repo), "");
}
