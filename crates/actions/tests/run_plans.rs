//! Runs action plans against throwaway fixture repos and checks the result
//! with git. The app's real executor lives in `crates/app`; this runner only
//! adds the fixture's isolation (no global or system git config).

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers panic so a broken fixture fails loudly"
)]

use std::process::Command;

use pretty_assertions::assert_eq;
use tgg_actions::{
    Action, CommitTarget, InputId, InputValue, MenuContext, MenuItem, Plan, ResetMode, Rev, Target,
    menu_for, validate_ref_name,
};
use tgg_git::{Failure, GitCommand};
use tgg_model::{HeadState, Oid};
use tgg_testkit::FixtureRepo;

/// Runs every command of `action`'s plan, stopping at the first failure.
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

fn oid(hex: &str) -> Oid {
    Oid::parse(hex).unwrap()
}

fn head_branch(repo: &FixtureRepo) -> String {
    repo.git(&["symbolic-ref", "--short", "HEAD"])
}

fn subjects(repo: &FixtureRepo, rev: &str) -> Vec<String> {
    repo.git(&["log", "--format=%s", rev, "--"])
        .lines()
        .map(str::to_owned)
        .collect()
}

/// `main`: Base, then `topic` with one commit, then one more commit on `main`.
fn diverged() -> (FixtureRepo, String) {
    let repo = FixtureRepo::new();
    repo.commit_file("a.txt", "a\n", "Base");
    repo.branch("topic");
    let topic = repo.commit_file("t.txt", "t\n", "Topic work");
    repo.checkout("main");
    repo.commit_file("m.txt", "m\n", "Main work");
    (repo, topic)
}

// ---- branches (S8, S12) ----

#[test]
fn checkout_branch_switches_head() {
    let repo = FixtureRepo::sample();
    ok(
        &repo,
        &Action::CheckoutBranch {
            name: "develop".into(),
        },
    );
    assert_eq!(head_branch(&repo), "develop");
}

#[test]
fn checkout_remote_branch_creates_a_tracking_branch() {
    let repo = FixtureRepo::sample();
    repo.git(&[
        "push",
        "--quiet",
        "origin",
        "develop:refs/heads/remote-only",
    ]);
    repo.git(&["fetch", "--quiet", "origin"]);
    ok(
        &repo,
        &Action::CheckoutRemoteBranch {
            remote: "origin".into(),
            branch: "remote-only".into(),
            local_name: "remote-only".into(),
        },
    );
    assert_eq!(head_branch(&repo), "remote-only");
    assert_eq!(
        repo.git(&["rev-parse", "--abbrev-ref", "remote-only@{upstream}"]),
        "origin/remote-only"
    );
}

#[test]
fn create_branch_at_a_commit_with_and_without_checkout() {
    let (repo, topic) = diverged();
    let mut action = Action::CreateBranch {
        at: oid(&topic),
        name: "from-topic".into(),
        checkout: false,
    };
    ok(&repo, &action);
    assert_eq!(repo.git(&["rev-parse", "from-topic"]), topic);
    assert_eq!(head_branch(&repo), "main");

    action
        .set_input(InputId::Name, InputValue::Text("switched".into()))
        .unwrap();
    action
        .set_input(InputId::Checkout, InputValue::Bool(true))
        .unwrap();
    ok(&repo, &action);
    assert_eq!(head_branch(&repo), "switched");
    assert_eq!(repo.git(&["rev-parse", "HEAD"]), topic);
}

#[test]
fn rename_branch() {
    let (repo, topic) = diverged();
    ok(
        &repo,
        &Action::RenameBranch {
            from: "topic".into(),
            to: "renamed".into(),
        },
    );
    assert_eq!(repo.git(&["rev-parse", "renamed"]), topic);
    assert_eq!(repo.git(&["branch", "--list", "topic"]), "");
}

#[test]
fn delete_refuses_unmerged_branches_until_forced() {
    let (repo, _) = diverged();
    let mut action = Action::DeleteBranch {
        name: "topic".into(),
        force: false,
    };
    assert_eq!(run(&repo, &action), Err(Failure::NotFullyMerged));
    assert_eq!(repo.git(&["branch", "--list", "topic"]), "  topic");

    action
        .set_input(InputId::Force, InputValue::Bool(true))
        .unwrap();
    ok(&repo, &action);
    assert_eq!(repo.git(&["branch", "--list", "topic"]), "");
}

#[test]
fn delete_merged_branch_without_force() {
    let repo = FixtureRepo::new();
    repo.commit_file("a.txt", "a\n", "Base");
    repo.git(&["branch", "merged"]);
    ok(
        &repo,
        &Action::DeleteBranch {
            name: "merged".into(),
            force: false,
        },
    );
    assert_eq!(repo.git(&["branch", "--list", "merged"]), "");
}

#[test]
fn merge_into_current_creates_a_merge_commit() {
    let (repo, topic) = diverged();
    ok(
        &repo,
        &Action::MergeIntoCurrent {
            source: Rev::Local("topic".into()),
            current: Some("main".into()),
            no_ff: true,
        },
    );
    assert_eq!(
        repo.git(&["log", "-1", "--format=%s"]),
        "Merge branch 'topic'"
    );
    assert_eq!(repo.git(&["rev-parse", "HEAD^2"]), topic);
}

#[test]
fn rebase_current_branch_onto_another() {
    let (repo, _) = diverged();
    repo.checkout("topic");
    ok(
        &repo,
        &Action::RebaseCurrentOnto {
            onto: Rev::Local("main".into()),
            current: Some("topic".into()),
        },
    );
    assert_eq!(subjects(&repo, "HEAD"), ["Topic work", "Main work", "Base"]);
}

#[test]
fn push_sets_upstream_and_force_with_lease_rewrites_the_remote() {
    let (repo, _) = diverged();
    repo.add_origin();
    let mut push = Action::PushBranch {
        branch: "topic".into(),
        remote: "origin".into(),
        set_upstream: true,
        force_with_lease: false,
    };
    ok(&repo, &push);
    assert_eq!(
        repo.git(&["rev-parse", "--abbrev-ref", "topic@{upstream}"]),
        "origin/topic"
    );

    // Rewrite topic so the remote has a commit the local branch doesn't.
    repo.checkout("topic");
    repo.git(&[
        "commit",
        "--quiet",
        "--amend",
        "--message",
        "Topic work, amended",
    ]);
    assert_eq!(run(&repo, &push), Err(Failure::NonFastForward));

    push.set_input(InputId::ForceWithLease, InputValue::Bool(true))
        .unwrap();
    ok(&repo, &push);
    assert_eq!(
        repo.git(&["rev-parse", "origin/topic"]),
        repo.git(&["rev-parse", "topic"])
    );
}

fn remote_tip(repo: &FixtureRepo, branch: &str) -> String {
    let line = repo.git(&["ls-remote", "origin", &format!("refs/heads/{branch}")]);
    line.split_whitespace().next().unwrap().to_owned()
}

#[test]
fn fetch_into_a_local_branch_moves_it_to_the_remote_commit() {
    let repo = FixtureRepo::sample();
    repo.push("develop");
    let remote = remote_tip(&repo, "develop");
    repo.git(&["branch", "--force", "develop", "develop~1"]);
    let mut fetch = Action::FetchIntoLocal {
        remote: "origin".into(),
        branch: "develop".into(),
        local_name: "develop".into(),
        force: false,
    };
    ok(&repo, &fetch);
    assert_eq!(
        repo.git(&["rev-parse", "develop"]),
        remote,
        "fast-forwarded"
    );
    assert_eq!(head_branch(&repo), "main", "nothing was checked out");

    // A new name creates the branch.
    let mut copy = fetch.clone();
    copy.set_input(InputId::Name, InputValue::Text("develop-copy".into()))
        .unwrap();
    ok(&repo, &copy);
    assert_eq!(repo.git(&["rev-parse", "develop-copy"]), remote);

    // A local branch ahead of the remote isn't rewound without force.
    let login = repo.git(&["rev-parse", "feature/login"]);
    repo.git(&["branch", "--force", "develop", &login]);
    assert_eq!(run(&repo, &fetch), Err(Failure::NonFastForward));
    assert_eq!(repo.git(&["rev-parse", "develop"]), login);
    fetch
        .set_input(InputId::Force, InputValue::Bool(true))
        .unwrap();
    ok(&repo, &fetch);
    assert_eq!(repo.git(&["rev-parse", "develop"]), remote, "forced");
}

#[test]
fn fetch_into_the_checked_out_branch_fails_readably() {
    let repo = FixtureRepo::sample();
    let into_main = Action::FetchIntoLocal {
        remote: "origin".into(),
        branch: "main".into(),
        local_name: "main".into(),
        force: false,
    };
    let message = run(&repo, &into_main).unwrap_err().message();
    assert!(message.contains("main"), "{message}");
}

#[test]
fn create_archive_writes_a_zip() {
    let (repo, _) = diverged();
    ok(
        &repo,
        &Action::CreateArchive {
            rev: Rev::Local("topic".into()),
            output: "topic.zip".into(),
        },
    );
    let bytes = std::fs::read(repo.path().join("topic.zip")).unwrap();
    assert_eq!(&bytes[..2], b"PK", "zip magic");
}

// ---- commits (S9) ----

#[test]
fn add_lightweight_and_annotated_tags() {
    let (repo, topic) = diverged();
    let mut tag = Action::AddTag {
        at: oid(&topic),
        name: "annotated".into(),
        annotated: true,
        message: String::new(),
        remote: None,
        push: false,
    };
    // An annotated tag with no message is still annotated.
    ok(&repo, &tag);
    assert_eq!(repo.git(&["cat-file", "-t", "annotated"]), "tag");
    assert_eq!(repo.git(&["rev-parse", "annotated^{commit}"]), topic);

    tag.set_input(InputId::Name, InputValue::Text("light".into()))
        .unwrap();
    tag.set_input(InputId::TagType, InputValue::Text("Lightweight".into()))
        .unwrap();
    ok(&repo, &tag);
    assert_eq!(repo.git(&["cat-file", "-t", "light"]), "commit");
}

#[test]
fn add_tag_and_push_it_to_the_remote() {
    let (repo, topic) = diverged();
    repo.add_origin();
    let mut tag = Action::AddTag {
        at: oid(&topic),
        name: "v1.0".into(),
        annotated: true,
        message: "Release".into(),
        remote: Some("origin".into()),
        push: false,
    };
    tag.set_input(InputId::PushToRemote, InputValue::Bool(true))
        .unwrap();
    ok(&repo, &tag);
    let local = repo.git(&["rev-parse", "v1.0"]);
    assert!(
        repo.git(&["ls-remote", "--tags", "origin"])
            .contains(&format!("{local}\trefs/tags/v1.0")),
    );
}

#[test]
fn checkout_commit_detaches_head() {
    let (repo, topic) = diverged();
    ok(
        &repo,
        &Action::CheckoutCommit {
            commit: oid(&topic),
            subject: "Topic work".into(),
        },
    );
    assert_eq!(repo.git(&["rev-parse", "HEAD"]), topic);
    assert_eq!(repo.git(&["branch", "--show-current"]), "");
}

#[test]
fn cherry_pick_copies_a_commit_onto_the_current_branch() {
    let (repo, topic) = diverged();
    ok(
        &repo,
        &Action::CherryPick {
            commit: oid(&topic),
            subject: "Topic work".into(),
            is_merge: false,
        },
    );
    assert_eq!(subjects(&repo, "HEAD")[0], "Topic work");
    assert!(repo.path().join("t.txt").exists());
}

#[test]
fn cherry_pick_and_revert_a_merge_relative_to_its_first_parent() {
    let repo = FixtureRepo::sample();
    let merge = repo.git(&["rev-parse", "main"]);
    ok(
        &repo,
        &Action::Revert {
            commit: oid(&merge),
            subject: "Merge branch 'develop'".into(),
            is_merge: true,
        },
    );
    assert_eq!(
        repo.git(&["log", "-1", "--format=%s"]),
        "Revert \"Merge branch 'develop'\""
    );
    assert!(!repo.path().join("src/lib.rs").exists());
}

#[test]
fn revert_undoes_a_commit() {
    let (repo, _) = diverged();
    let main_work = repo.git(&["rev-parse", "HEAD"]);
    ok(
        &repo,
        &Action::Revert {
            commit: oid(&main_work),
            subject: "Main work".into(),
            is_merge: false,
        },
    );
    assert_eq!(subjects(&repo, "HEAD")[0], "Revert \"Main work\"");
    assert!(!repo.path().join("m.txt").exists());
}

#[test]
fn drop_removes_a_commit_from_the_current_branch() {
    let repo = FixtureRepo::new();
    repo.commit_file("a.txt", "a\n", "First");
    let second = repo.commit_file("b.txt", "b\n", "Second");
    repo.commit_file("c.txt", "c\n", "Third");
    ok(
        &repo,
        &Action::Drop {
            commit: oid(&second),
            subject: "Second".into(),
            current: Some("main".into()),
        },
    );
    assert_eq!(subjects(&repo, "HEAD"), ["Third", "First"]);
    assert_eq!(head_branch(&repo), "main");
    assert!(!repo.path().join("b.txt").exists());
}

#[test]
fn reset_modes_keep_or_discard_changes() {
    let build = || {
        let repo = FixtureRepo::new();
        let first = repo.commit_file("a.txt", "1\n", "First");
        repo.commit_file("a.txt", "2\n", "Second");
        (repo, first)
    };
    let reset = |first: &str, mode| Action::ResetCurrentTo {
        commit: oid(first),
        subject: "First".into(),
        current: "main".into(),
        mode,
    };
    let status = |repo: &FixtureRepo| repo.git(&["status", "--porcelain"]);

    let (repo, first) = build();
    ok(&repo, &reset(&first, ResetMode::Soft));
    assert_eq!(status(&repo), "M  a.txt", "soft keeps the change staged");

    let (repo, first) = build();
    ok(&repo, &reset(&first, ResetMode::Mixed));
    assert_eq!(status(&repo), " M a.txt", "mixed keeps it unstaged");

    let (repo, first) = build();
    ok(&repo, &reset(&first, ResetMode::Hard));
    assert_eq!(status(&repo), "", "hard discards it");
    assert_eq!(repo.git(&["rev-parse", "HEAD"]), first);
}

// ---- menus drive real actions ----

#[test]
fn every_git_entry_of_the_sample_menus_plans_valid_git() {
    let repo = FixtureRepo::sample();
    let ctx = MenuContext {
        head: HeadState::Branch("main".into()),
        head_commit: Some(oid(&repo.git(&["rev-parse", "HEAD"]))),
        remotes: vec!["origin".into()],
        branch_filter: Vec::new(),
        compare_base: None,
        operation: None,
    };
    let login = repo.git(&["rev-parse", "feature/login"]);
    let targets = [
        Target::LocalBranch {
            name: "feature/login".into(),
            target: oid(&login),
        },
        Target::Commit(CommitTarget {
            id: oid(&login),
            subject: "Add login".into(),
            parent_count: 1,
            is_head_ancestor: false,
            linear_to_head: false,
        }),
    ];
    for target in &targets {
        for item in menu_for(target, &ctx) {
            let MenuItem::Entry(entry) = item else {
                continue;
            };
            // Prefilled empty names are what the dialog asks for.
            let mut action = entry.action;
            if action.validate().is_err() {
                action
                    .set_input(InputId::Name, InputValue::Text("from-menu".into()))
                    .unwrap();
            }
            assert_eq!(action.validate(), Ok(()), "{}", entry.label);
            if let Plan::Git(commands) = action.plan(repo.path()) {
                assert!(!commands.is_empty(), "{}", entry.label);
            }
        }
    }
}

#[test]
fn local_name_check_agrees_with_git() {
    let repo = FixtureRepo::new();
    let names = [
        "main",
        "feature/login",
        "v1.0",
        "ünïcode",
        "",
        "-x",
        "a b",
        "a..b",
        "a~1",
        "a^",
        "a:b",
        "a?",
        "a*",
        "a[b",
        "a\\b",
        "a@{1}",
        "HEAD",
        ".hidden",
        "a/.b",
        "a.lock",
        "/a",
        "a/",
        "a//b",
        "a.",
    ];
    for name in names {
        let by_git = Command::new("git")
            .args(["-C", &repo.path().to_string_lossy()])
            .args(["check-ref-format", "--branch", name])
            .output()
            .unwrap()
            .status
            .success();
        assert_eq!(
            validate_ref_name(name).is_ok(),
            by_git,
            "{name:?}: ours vs git check-ref-format --branch"
        );
    }
    // Deliberately stricter than git: a branch named `@` is legal, but `@`
    // means HEAD in every revision argument.
    assert!(validate_ref_name("@").is_err());
}
