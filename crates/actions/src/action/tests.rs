use std::path::Path;

use ggt_git::{CommandKind, GitCommand};
use ggt_model::Oid;
use pretty_assertions::assert_eq;

use super::*;
use crate::{InputError, InputId, InputKind, InputSpec, InputValue, RefNameError};

const HASH: &str = "0123456789abcdef0123456789abcdef01234567";

fn oid() -> Oid {
    Oid::parse(HASH).unwrap()
}

fn repo() -> &'static Path {
    Path::new("/repo")
}

/// The single command of a plan, as `(kind, args after the -C/-c prefix)`.
fn single(action: &Action) -> (CommandKind, Vec<String>) {
    let Plan::Git(commands) = action.plan(repo()) else {
        panic!("expected git commands for {action:?}");
    };
    assert_eq!(commands.len(), 1, "{action:?}");
    let command: &GitCommand = &commands[0];
    (command.kind(), command.argv()[6..].to_vec())
}

fn args(action: &Action) -> Vec<String> {
    single(action).1
}

fn write(argv: &[&str]) -> (CommandKind, Vec<String>) {
    (
        CommandKind::Write,
        argv.iter().map(|a| (*a).to_owned()).collect(),
    )
}

fn local(name: &str) -> Rev {
    Rev::Local(name.into())
}

// ---- plans ----

#[test]
fn checkout_branch_switches_to_it() {
    let action = Action::CheckoutBranch {
        name: "develop".into(),
    };
    assert_eq!(single(&action), write(&["switch", "--", "develop"]));
}

#[test]
fn checkout_remote_branch_creates_a_tracking_branch() {
    let action = Action::CheckoutRemoteBranch {
        remote: "origin".into(),
        branch: "feature/x".into(),
        local_name: "feature/x".into(),
    };
    assert_eq!(
        single(&action),
        write(&[
            "switch",
            "--create",
            "feature/x",
            "--track",
            "--",
            "origin/feature/x"
        ])
    );
}

#[test]
fn create_branch_with_and_without_checkout() {
    let mut action = Action::CreateBranch {
        at: oid(),
        name: "topic".into(),
        checkout: false,
    };
    assert_eq!(single(&action), write(&["branch", "--", "topic", HASH]));
    action
        .set_input(InputId::Checkout, InputValue::Bool(true))
        .unwrap();
    assert_eq!(
        single(&action),
        write(&["switch", "--create", "topic", "--", HASH])
    );
}

#[test]
fn rename_branch() {
    let action = Action::RenameBranch {
        from: "old".into(),
        to: "new".into(),
    };
    assert_eq!(
        single(&action),
        write(&["branch", "--move", "--", "old", "new"])
    );
}

#[test]
fn delete_branch_is_safe_by_default() {
    let mut action = Action::DeleteBranch {
        name: "topic".into(),
        force: false,
    };
    assert_eq!(
        single(&action),
        write(&["branch", "--delete", "--", "topic"])
    );
    action
        .set_input(InputId::Force, InputValue::Bool(true))
        .unwrap();
    assert_eq!(
        single(&action),
        write(&["branch", "--delete", "--force", "--", "topic"])
    );
}

#[test]
fn merge_creates_a_merge_commit_unless_fast_forward_is_allowed() {
    let mut action = Action::MergeIntoCurrent {
        source: local("develop"),
        current: Some("main".into()),
        no_ff: true,
        squash: false,
        no_commit: false,
    };
    assert_eq!(
        single(&action),
        write(&["merge", "--no-edit", "--no-ff", "--", "develop"])
    );
    action
        .set_input(InputId::NoFastForward, InputValue::Bool(false))
        .unwrap();
    assert_eq!(
        single(&action),
        write(&["merge", "--no-edit", "--", "develop"])
    );
}

fn merge(no_ff: bool, squash: bool, no_commit: bool) -> Action {
    Action::MergeIntoCurrent {
        source: local("develop"),
        current: Some("main".into()),
        no_ff,
        squash,
        no_commit,
    }
}

#[test]
fn merge_without_commit_stops_before_committing() {
    assert_eq!(
        single(&merge(true, false, true)),
        write(&[
            "merge",
            "--no-edit",
            "--no-ff",
            "--no-commit",
            "--",
            "develop"
        ])
    );
}

#[test]
fn squash_merge_commits_the_staged_result_and_ignores_no_ff() {
    let Plan::Git(commands) = merge(true, true, false).plan(repo()) else {
        panic!("expected git commands");
    };
    let argvs: Vec<Vec<String>> = commands.iter().map(|c| c.argv()[6..].to_vec()).collect();
    assert_eq!(
        argvs,
        vec![
            strings(&["merge", "--squash", "--", "develop"]),
            strings(&["commit", "--no-edit"]),
        ]
    );
    assert_eq!(
        single(&merge(true, true, true)),
        write(&["merge", "--squash", "--", "develop"])
    );
}

#[test]
fn merge_dialog_asks_like_vs_code() {
    let mut action = merge(true, false, false);
    assert_eq!(
        action.question().as_deref(),
        Some("Are you sure you want to merge branch 'develop' into 'main' (the current branch)?")
    );
    assert_eq!(action.confirm_label(), Some("Yes, merge"));
    let labels: Vec<(InputId, &str)> = action.inputs().iter().map(|i| (i.id, i.label)).collect();
    assert_eq!(
        labels,
        vec![
            (
                InputId::NoFastForward,
                "Create a new commit even if fast-forward is possible"
            ),
            (InputId::Squash, "Squash Commits"),
            (InputId::NoCommit, "No Commit"),
        ]
    );
    action
        .set_input(InputId::Squash, InputValue::Bool(true))
        .unwrap();
    action
        .set_input(InputId::NoCommit, InputValue::Bool(true))
        .unwrap();
    assert_eq!(action, merge(true, true, true));
}

#[test]
fn remote_checkout_dialog_asks_for_the_new_branch_like_vs_code() {
    let action = Action::CheckoutRemoteBranch {
        remote: "origin".into(),
        branch: "development".into(),
        local_name: "development".into(),
    };
    assert_eq!(
        action.question().as_deref(),
        Some(
            "Enter the name of the new branch you would like to create when checking out 'origin/development':"
        )
    );
    assert_eq!(action.confirm_label(), Some("Checkout Branch"));
}

#[test]
fn rebase_current_branch() {
    let action = Action::RebaseCurrentOnto {
        onto: Rev::Remote {
            remote: "origin".into(),
            branch: "main".into(),
        },
        current: Some("topic".into()),
    };
    assert_eq!(single(&action), write(&["rebase", "--", "origin/main"]));
}

#[test]
fn push_is_a_network_command_with_an_explicit_refspec() {
    let mut action = Action::PushBranch {
        branch: "topic".into(),
        remote: "origin".into(),
        set_upstream: true,
        force_with_lease: false,
    };
    let refspec = "refs/heads/topic:refs/heads/topic";
    assert_eq!(
        single(&action),
        (
            CommandKind::Network,
            vec![
                "push".into(),
                "--set-upstream".into(),
                "--".into(),
                "origin".into(),
                refspec.into()
            ]
        )
    );
    action
        .set_input(InputId::SetUpstream, InputValue::Bool(false))
        .unwrap();
    action
        .set_input(InputId::ForceWithLease, InputValue::Bool(true))
        .unwrap();
    assert_eq!(
        args(&action),
        ["push", "--force-with-lease", "--", "origin", refspec]
    );
}

fn fetch_into(force: bool) -> Action {
    Action::FetchIntoLocal {
        remote: "origin".into(),
        branch: "feature/x".into(),
        local_name: "feature/x".into(),
        force,
    }
}

#[test]
fn fetch_into_a_local_branch_is_a_network_command_with_an_explicit_refspec() {
    let mut action = fetch_into(false);
    assert_eq!(
        single(&action),
        (
            CommandKind::Network,
            vec![
                "fetch".into(),
                "--".into(),
                "origin".into(),
                "refs/heads/feature/x:refs/heads/feature/x".into()
            ]
        )
    );
    assert_eq!(
        action.inputs(),
        [
            InputSpec::text(
                InputId::Name,
                "Name of the local branch",
                InputKind::RefName,
                "feature/x"
            ),
            InputSpec::toggle(InputId::Force, "Force fetch", false),
        ]
    );
    action
        .set_input(InputId::Name, InputValue::Text("mine".into()))
        .unwrap();
    action
        .set_input(InputId::Force, InputValue::Bool(true))
        .unwrap();
    assert_eq!(
        args(&action),
        [
            "fetch",
            "--force",
            "--",
            "origin",
            "refs/heads/feature/x:refs/heads/mine"
        ]
    );
    assert_eq!(
        action.set_input(InputId::Name, InputValue::Text("bad name".into())),
        Err(InputError::InvalidName(RefNameError::Whitespace))
    );
}

#[test]
fn a_forced_fetch_into_a_local_branch_asks_first() {
    assert!(!fetch_into(false).is_destructive());
    assert_eq!(fetch_into(false).confirmation(), None);
    let forced = fetch_into(true);
    assert!(forced.is_destructive());
    assert_eq!(
        forced.confirmation().unwrap().message,
        "Force fetch origin/feature/x into the local branch 'feature/x'? Commits on 'feature/x' that aren't on origin/feature/x will be lost."
    );
}

#[test]
fn create_archive_lets_git_pick_the_format_from_the_extension() {
    let action = Action::CreateArchive {
        rev: local("main"),
        output: "main.zip".into(),
    };
    assert_eq!(
        single(&action),
        write(&["archive", "--output=main.zip", "--", "main"])
    );
}

fn add_tag(remote: Option<&str>) -> Action {
    Action::AddTag {
        at: oid(),
        name: "v1.0".into(),
        annotated: true,
        message: String::new(),
        remote: remote.map(str::to_owned),
        push: false,
    }
}

fn annotated(message: &str) -> (CommandKind, Vec<String>) {
    write(&[
        "tag",
        "--annotate",
        "--message",
        message,
        "--",
        "v1.0",
        HASH,
    ])
}

#[test]
fn add_tag_is_annotated_unless_lightweight_is_chosen() {
    let mut action = add_tag(None);
    // As in VS Code, an annotated tag's message is optional.
    assert_eq!(single(&action), annotated(""));
    action
        .set_input(InputId::Message, InputValue::Text("Release".into()))
        .unwrap();
    assert_eq!(single(&action), annotated("Release"));
    action
        .set_input(InputId::TagType, InputValue::Text("Lightweight".into()))
        .unwrap();
    assert_eq!(single(&action), write(&["tag", "--", "v1.0", HASH]));
    assert_eq!(
        action.set_input(InputId::TagType, InputValue::Text("Signed".into())),
        Err(InputError::NotAChoice("Signed".into()))
    );
}

#[test]
fn add_tag_can_push_it_to_the_remote() {
    let mut action = add_tag(Some("origin"));
    action
        .set_input(InputId::PushToRemote, InputValue::Bool(true))
        .unwrap();
    let Plan::Git(commands) = action.plan(repo()) else {
        panic!("expected git commands");
    };
    let steps: Vec<_> = commands
        .iter()
        .map(|c| (c.kind(), c.argv()[6..].to_vec()))
        .collect();
    assert_eq!(
        steps,
        [
            annotated(""),
            (
                CommandKind::Network,
                strings(&["push", "--", "origin", "refs/tags/v1.0"])
            ),
        ]
    );
    // Without a remote there's nothing to push to.
    assert_eq!(
        add_tag(None).set_input(InputId::PushToRemote, InputValue::Bool(true)),
        Err(InputError::NotAnInput(InputId::PushToRemote))
    );
}

#[test]
fn checkout_commit_detaches_head() {
    let action = Action::CheckoutCommit {
        commit: oid(),
        subject: "Fix".into(),
    };
    assert_eq!(single(&action), write(&["switch", "--detach", "--", HASH]));
}

#[test]
fn cherry_pick_and_revert_use_the_first_parent_for_merges() {
    let pick = |is_merge| Action::CherryPick {
        commit: oid(),
        subject: "s".into(),
        is_merge,
    };
    assert_eq!(single(&pick(false)), write(&["cherry-pick", "--", HASH]));
    assert_eq!(
        args(&pick(true)),
        ["cherry-pick", "--mainline", "1", "--", HASH]
    );

    let revert = |is_merge| Action::Revert {
        commit: oid(),
        subject: "s".into(),
        is_merge,
    };
    assert_eq!(
        single(&revert(false)),
        write(&["revert", "--no-edit", "--", HASH])
    );
    assert_eq!(
        args(&revert(true)),
        ["revert", "--no-edit", "--mainline", "1", "--", HASH]
    );
}

#[test]
fn drop_rebases_the_later_commits_onto_the_parent() {
    let action = Action::Drop {
        commit: oid(),
        subject: "Oops".into(),
        current: Some("main".into()),
    };
    let parent = format!("{HASH}^");
    assert_eq!(
        single(&action),
        write(&["rebase", "--onto", &parent, "--", HASH])
    );
}

#[test]
fn reset_defaults_to_mixed_and_puts_the_commit_before_the_double_dash() {
    // After a revision, `--` ends the revisions: `reset -- <commit>` would
    // treat the commit as a path.
    let mut action = Action::ResetCurrentTo {
        commit: oid(),
        subject: "s".into(),
        current: "main".into(),
        mode: ResetMode::default(),
    };
    assert_eq!(single(&action), write(&["reset", "--mixed", HASH, "--"]));
    action
        .set_input(InputId::Mode, InputValue::Text("hard".into()))
        .unwrap();
    assert_eq!(args(&action), ["reset", "--hard", HASH, "--"]);
}

#[test]
fn copy_actions_plan_clipboard_text() {
    let copy = |action: Action| action.plan(repo());
    assert_eq!(
        copy(Action::CopyBranchName {
            name: "origin/main".into()
        }),
        Plan::Clipboard("origin/main".into())
    );
    assert_eq!(
        copy(Action::CopyCommitHash { commit: oid() }),
        Plan::Clipboard(HASH.into())
    );
    assert_eq!(
        copy(Action::CopyCommitSubject {
            subject: "Fix it".into()
        }),
        Plan::Clipboard("Fix it".into())
    );
}

#[test]
fn branch_filter_toggle_is_a_ui_effect() {
    let action = Action::ToggleBranchFilter {
        branch: "develop".into(),
        selected: false,
    };
    assert_eq!(
        action.plan(repo()),
        Plan::Ui(UiEffect::ToggleBranchFilter {
            branch: "develop".into()
        })
    );
}

#[test]
fn user_input_never_becomes_an_option() {
    // Validation rejects it, but even an unvalidated name stays after `--`.
    let action = Action::DeleteBranch {
        name: "--force".into(),
        force: false,
    };
    assert_eq!(args(&action), ["branch", "--delete", "--", "--force"]);
}

// ---- destructive actions and confirmations (S13) ----

fn delete(force: bool) -> Action {
    Action::DeleteBranch {
        name: "topic".into(),
        force,
    }
}

fn reset(mode: ResetMode) -> Action {
    Action::ResetCurrentTo {
        commit: oid(),
        subject: "Add lib".into(),
        current: "main".into(),
        mode,
    }
}

fn push(force_with_lease: bool) -> Action {
    Action::PushBranch {
        branch: "topic".into(),
        remote: "origin".into(),
        set_upstream: false,
        force_with_lease,
    }
}

#[test]
fn destructive_actions() {
    let destructive = [
        delete(false),
        delete(true),
        reset(ResetMode::Hard),
        push(true),
        Action::Drop {
            commit: oid(),
            subject: "s".into(),
            current: Some("main".into()),
        },
        Action::RebaseCurrentOnto {
            onto: local("develop"),
            current: Some("main".into()),
        },
    ];
    for action in &destructive {
        assert!(action.is_destructive(), "{action:?}");
        assert!(action.confirmation().is_some(), "{action:?}");
    }
    for action in [
        push(false),
        reset(ResetMode::Mixed),
        Action::CheckoutBranch {
            name: "develop".into(),
        },
        Action::CheckoutCommit {
            commit: oid(),
            subject: "s".into(),
        },
    ] {
        assert!(!action.is_destructive(), "{action:?}");
    }
}

#[test]
fn safe_actions_need_no_confirmation() {
    for action in [
        push(false),
        Action::CheckoutBranch {
            name: "develop".into(),
        },
        Action::CopyCommitHash { commit: oid() },
    ] {
        assert_eq!(action.confirmation(), None, "{action:?}");
    }
}

fn message(action: &Action) -> String {
    action.confirmation().unwrap().message
}

#[test]
fn delete_confirmation_names_the_branch_and_the_risk() {
    assert_eq!(
        message(&delete(false)),
        "Delete the local branch 'topic'? git refuses if it isn't fully merged."
    );
    assert_eq!(
        message(&delete(true)),
        "Force delete the local branch 'topic'? Commits that are only on this branch will be lost."
    );
}

#[test]
fn reset_confirmation_explains_each_mode() {
    assert_eq!(
        message(&reset(ResetMode::Soft)),
        "Reset 'main' to commit 0123456 \"Add lib\" (soft)? Later commits leave 'main'; their changes stay staged."
    );
    assert_eq!(
        message(&reset(ResetMode::Mixed)),
        "Reset 'main' to commit 0123456 \"Add lib\" (mixed)? Later commits leave 'main'; their changes stay in the working tree, unstaged."
    );
    assert_eq!(
        message(&reset(ResetMode::Hard)),
        "Hard reset 'main' to commit 0123456 \"Add lib\"? Later commits leave 'main' and ALL uncommitted changes are discarded."
    );
}

#[test]
fn force_push_confirmation_mentions_the_lease() {
    assert_eq!(
        message(&push(true)),
        "Force push 'topic' to 'origin'? Commits on origin/topic that aren't in your 'topic' will be overwritten. The push is refused if origin/topic changed since your last fetch (--force-with-lease)."
    );
}

#[test]
fn detached_checkout_confirmation_explains_detached_head() {
    let action = Action::CheckoutCommit {
        commit: oid(),
        subject: "Fix".into(),
    };
    assert_eq!(
        message(&action),
        "Check out commit 0123456 \"Fix\"? HEAD will be detached: new commits won't belong to any branch until you create one."
    );
}

#[test]
fn drop_confirmation_says_history_is_rewritten() {
    let action = Action::Drop {
        commit: oid(),
        subject: "Oops".into(),
        current: Some("main".into()),
    };
    assert_eq!(
        message(&action),
        "Drop commit 0123456 \"Oops\" from 'main'? The commits after it are rebased onto its parent (git rebase --onto 0123456^ 0123456), which rewrites the history of 'main'."
    );
}

#[test]
fn rebase_confirmation_names_both_sides() {
    let action = Action::RebaseCurrentOnto {
        onto: Rev::Commit(oid()),
        current: Some("main".into()),
    };
    assert_eq!(
        message(&action),
        "Rebase 'main' on commit 0123456? The commits of 'main' that aren't in commit 0123456 are rewritten on top of it."
    );
}

// ---- inputs ----

#[test]
fn inputs_describe_what_each_dialog_asks() {
    let ids = |action: Action| -> Vec<InputId> { action.inputs().iter().map(|i| i.id).collect() };
    assert_eq!(
        ids(Action::CreateBranch {
            at: oid(),
            name: String::new(),
            checkout: false
        }),
        [InputId::Name, InputId::Checkout]
    );
    assert_eq!(
        ids(add_tag(None)),
        [InputId::Name, InputId::TagType, InputId::Message]
    );
    assert_eq!(
        ids(add_tag(Some("origin"))),
        [
            InputId::Name,
            InputId::TagType,
            InputId::Message,
            InputId::PushToRemote
        ]
    );
    assert_eq!(
        ids(push(false)),
        [
            InputId::Remote,
            InputId::SetUpstream,
            InputId::ForceWithLease
        ]
    );
    assert_eq!(ids(reset(ResetMode::Mixed)), [InputId::Mode]);
    assert_eq!(ids(delete(false)), [InputId::Force]);
    assert!(ids(Action::CheckoutBranch { name: "x".into() }).is_empty());
}

#[test]
fn input_values_start_from_the_current_fields() {
    let inputs = reset(ResetMode::Mixed).inputs();
    assert_eq!(inputs[0].value, InputValue::Text("mixed".into()));
}

#[test]
fn names_are_validated_when_set() {
    let mut action = Action::CreateBranch {
        at: oid(),
        name: String::new(),
        checkout: false,
    };
    assert_eq!(
        action.set_input(InputId::Name, InputValue::Text("bad name".into())),
        Err(InputError::InvalidName(RefNameError::Whitespace))
    );
    assert_eq!(
        action.validate(),
        Err(InputError::InvalidName(RefNameError::Empty))
    );
    action
        .set_input(InputId::Name, InputValue::Text("good".into()))
        .unwrap();
    assert_eq!(action.validate(), Ok(()));
}

#[test]
fn wrong_inputs_are_rejected() {
    let mut action = reset(ResetMode::Mixed);
    assert_eq!(
        action.set_input(InputId::Mode, InputValue::Text("medium".into())),
        Err(InputError::NotAChoice("medium".into()))
    );
    assert_eq!(
        action.set_input(InputId::Mode, InputValue::Bool(true)),
        Err(InputError::WrongType(InputId::Mode))
    );
    assert_eq!(
        action.set_input(InputId::Name, InputValue::Text("x".into())),
        Err(InputError::NotAnInput(InputId::Name))
    );
}

#[test]
fn archive_output_is_required() {
    let mut action = Action::CreateArchive {
        rev: local("main"),
        output: "main.zip".into(),
    };
    assert_eq!(
        action.set_input(InputId::Output, InputValue::Text("  ".into())),
        Err(InputError::Required(InputId::Output))
    );
}

// ---- while a merge, rebase, cherry-pick or revert is in progress ----

fn blocked(actions: &[Action]) {
    for action in actions {
        assert!(action.conflicts_with_operation(), "{action:?} should wait");
    }
}

#[test]
fn checkouts_wait_for_the_operation() {
    blocked(&[
        Action::CheckoutBranch {
            name: "develop".into(),
        },
        Action::CheckoutRemoteBranch {
            remote: "origin".into(),
            branch: "develop".into(),
            local_name: "develop".into(),
        },
        Action::CreateBranch {
            at: oid(),
            name: "x".into(),
            checkout: true,
        },
        Action::CheckoutCommit {
            commit: oid(),
            subject: "s".into(),
        },
        Action::Stash(StashAction::CreateBranch {
            stash: ggt_model::Stash {
                id: oid(),
                index: 0,
                subject: "WIP".into(),
            },
            name: "x".into(),
        }),
    ]);
}

#[test]
fn merges_wait_for_the_operation() {
    let stash = ggt_model::Stash {
        id: oid(),
        index: 0,
        subject: "WIP".into(),
    };
    blocked(&[
        Action::MergeIntoCurrent {
            source: local("develop"),
            current: Some("main".into()),
            no_ff: false,
            squash: false,
            no_commit: false,
        },
        Action::Stash(StashAction::Apply {
            stash: stash.clone(),
            reinstate_index: false,
        }),
        Action::Stash(StashAction::Pop {
            stash,
            reinstate_index: false,
        }),
        Action::Repo(RepoAction::ApplyLatestStash),
        Action::Repo(RepoAction::PopLatestStash),
    ]);
}

#[test]
fn rebases_wait_for_the_operation() {
    blocked(&[
        Action::RebaseCurrentOnto {
            onto: local("develop"),
            current: Some("main".into()),
        },
        Action::Drop {
            commit: oid(),
            subject: "s".into(),
            current: Some("main".into()),
        },
    ]);
}

#[test]
fn cherry_picks_and_reverts_wait_for_the_operation() {
    blocked(&[
        Action::CherryPick {
            commit: oid(),
            subject: "s".into(),
            is_merge: false,
        },
        Action::Revert {
            commit: oid(),
            subject: "s".into(),
            is_merge: false,
        },
    ]);
}

#[test]
fn resets_wait_for_the_operation() {
    blocked(&[
        reset(ResetMode::Soft),
        reset(ResetMode::Hard),
        Action::WorkingTree(WorkingTreeAction::Reset {
            mode: ResetMode::Mixed,
        }),
    ]);
}

#[test]
fn pulls_wait_for_the_operation() {
    blocked(&[
        Action::Repo(RepoAction::Sync),
        Action::Repo(RepoAction::PullUpstream { rebase: true }),
        Action::Repo(RepoAction::PullFrom {
            remote: "origin".into(),
            branch: "main".into(),
        }),
    ]);
}

#[test]
fn resolving_conflicts_stays_allowed() {
    let paths = vec!["file.txt".to_owned()];
    for action in [
        WorkingTreeAction::Stage {
            paths: paths.clone(),
        },
        WorkingTreeAction::Unstage {
            paths: paths.clone(),
        },
        WorkingTreeAction::Commit {
            message: "Merge".into(),
            all: false,
            amend: false,
            signoff: false,
        },
        WorkingTreeAction::Discard {
            paths,
            untracked: Vec::new(),
        },
    ] {
        let action = Action::WorkingTree(action);
        assert!(!action.conflicts_with_operation(), "{action:?}");
    }
    let others = [
        Action::CreateBranch {
            at: oid(),
            name: "x".into(),
            checkout: false,
        },
        Action::Repo(RepoAction::Fetch { prune: false }),
        Action::CopyCommitHash { commit: oid() },
    ];
    for action in others {
        assert!(!action.conflicts_with_operation(), "{action:?}");
    }
}
