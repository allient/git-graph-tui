use std::path::Path;

use pretty_assertions::assert_eq;

use super::*;
use crate::{InputError, InputId, InputValue, Plan, ResetMode, UiEffect};

fn args(action: &WorkingTreeAction) -> Vec<String> {
    let Plan::Git(commands) = action.plan(Path::new("/r")) else {
        panic!("expected git commands for {action:?}");
    };
    assert_eq!(commands.len(), 1);
    commands[0].argv()[6..].to_vec()
}

fn reset(mode: ResetMode) -> WorkingTreeAction {
    WorkingTreeAction::Reset { mode }
}

#[test]
fn stash_includes_untracked_files_and_an_optional_message() {
    let mut stash = WorkingTreeAction::Stash {
        message: String::new(),
        include_untracked: true,
    };
    assert_eq!(args(&stash), ["stash", "push", "--include-untracked"]);
    stash
        .set_input(InputId::Message, InputValue::Text("WIP: login".into()))
        .unwrap();
    stash
        .set_input(InputId::IncludeUntracked, InputValue::Bool(false))
        .unwrap();
    assert_eq!(args(&stash), ["stash", "push", "--message", "WIP: login"]);
}

#[test]
fn reset_and_clean() {
    assert_eq!(
        args(&reset(ResetMode::Mixed)),
        ["reset", "--mixed", "HEAD", "--"]
    );
    assert_eq!(
        args(&reset(ResetMode::Hard)),
        ["reset", "--hard", "HEAD", "--"]
    );
    assert_eq!(
        args(&WorkingTreeAction::Clean),
        ["clean", "-d", "--force", "--"]
    );
    assert_eq!(
        WorkingTreeAction::OpenSourceControl.plan(Path::new("/r")),
        Plan::Ui(UiEffect::OpenSourceControl)
    );
}

#[test]
fn hard_reset_and_clean_are_destructive() {
    assert!(reset(ResetMode::Hard).is_destructive());
    assert!(WorkingTreeAction::Clean.is_destructive());
    assert!(!reset(ResetMode::Mixed).is_destructive());
    assert_eq!(
        reset(ResetMode::Mixed).confirmation().unwrap().message,
        "Unstage all staged changes? The files keep their changes (git reset --mixed HEAD)."
    );
    assert_eq!(
        reset(ResetMode::Hard).confirmation().unwrap().message,
        "Discard ALL uncommitted changes to tracked files? This can't be undone (git reset --hard HEAD). Untracked files are kept."
    );
    assert_eq!(
        WorkingTreeAction::Clean.confirmation().unwrap().message,
        "Delete all untracked files and directories? This can't be undone (git clean -d --force). Ignored files are kept."
    );
    let stash = WorkingTreeAction::Stash {
        message: String::new(),
        include_untracked: true,
    };
    assert_eq!(stash.confirmation(), None);
}

#[test]
fn reset_offers_only_mixed_and_hard() {
    let mut action = reset(ResetMode::Mixed);
    assert_eq!(action.inputs()[0].id, InputId::Mode);
    assert_eq!(
        action.set_input(InputId::Mode, InputValue::Text("soft".into())),
        Err(InputError::NotAChoice("soft".into()))
    );
    action
        .set_input(InputId::Mode, InputValue::Text("hard".into()))
        .unwrap();
    assert_eq!(action, reset(ResetMode::Hard));
}

#[test]
fn stage_unstage_and_commit_use_literal_paths_and_one_message_argument() {
    let paths = vec!["a.rs".to_owned(), "*.md".to_owned(), "-x".to_owned()];
    assert_eq!(
        args(&WorkingTreeAction::Stage {
            paths: paths.clone()
        }),
        [
            "add",
            "--all",
            "--",
            ":(literal)a.rs",
            ":(literal)*.md",
            ":(literal)-x"
        ]
    );
    assert_eq!(
        args(&WorkingTreeAction::Unstage { paths }),
        [
            "reset",
            "--quiet",
            "--",
            ":(literal)a.rs",
            ":(literal)*.md",
            ":(literal)-x"
        ]
    );
    assert_eq!(
        args(&WorkingTreeAction::Commit {
            message: "-m: fix it".into()
        }),
        ["commit", "--message=-m: fix it"]
    );
}

#[test]
fn stage_unstage_and_commit_need_no_dialog() {
    for action in [
        WorkingTreeAction::Stage { paths: Vec::new() },
        WorkingTreeAction::Unstage { paths: Vec::new() },
        WorkingTreeAction::Commit {
            message: "x".into(),
        },
    ] {
        assert!(!action.is_destructive(), "{action:?}");
        assert!(action.confirmation().is_none(), "{action:?}");
        assert!(action.inputs().is_empty(), "{action:?}");
    }
}

fn all_args(action: &WorkingTreeAction) -> Vec<Vec<String>> {
    let Plan::Git(commands) = action.plan(Path::new("/r")) else {
        panic!("expected git commands for {action:?}");
    };
    commands.iter().map(|c| c.argv()[6..].to_vec()).collect()
}

#[test]
fn discard_restores_tracked_files_and_deletes_untracked_ones() {
    let discard = WorkingTreeAction::Discard {
        paths: vec!["a.rs".into()],
        untracked: vec!["new file.txt".into()],
    };
    assert_eq!(
        all_args(&discard),
        [
            vec!["restore", "--worktree", "--", ":(literal)a.rs"],
            vec!["clean", "--force", "--", ":(literal)new file.txt"],
        ]
    );
    let tracked_only = WorkingTreeAction::Discard {
        paths: vec!["a.rs".into()],
        untracked: Vec::new(),
    };
    assert_eq!(all_args(&tracked_only).len(), 1);
}

#[test]
fn discard_is_destructive_and_names_what_it_loses() {
    let one = WorkingTreeAction::Discard {
        paths: vec!["src/a.rs".into()],
        untracked: Vec::new(),
    };
    assert!(one.is_destructive());
    let confirmation = one.confirmation().unwrap();
    assert_eq!(confirmation.title, "Discard Changes");
    assert_eq!(
        confirmation.message,
        "Discard the changes in 'src/a.rs'? This can't be undone."
    );
    let untracked = WorkingTreeAction::Discard {
        paths: Vec::new(),
        untracked: vec!["notes.txt".into()],
    };
    assert_eq!(
        untracked.confirmation().unwrap().message,
        "Delete the untracked file 'notes.txt'? This can't be undone."
    );
    let many = WorkingTreeAction::Discard {
        paths: vec!["a".into(), "b".into()],
        untracked: vec!["c".into()],
    };
    assert_eq!(
        many.confirmation().unwrap().message,
        "Discard the changes in 3 files? Untracked files are deleted. This can't be undone."
    );
}
