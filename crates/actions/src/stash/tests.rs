use std::path::Path;

use pretty_assertions::assert_eq;
use tgg_model::{Oid, Stash};

use super::*;
use crate::{InputId, InputValue, Plan};

fn stash() -> Stash {
    Stash {
        id: Oid::parse(&"d".repeat(40)).unwrap(),
        index: 1,
        subject: "On main: WIP".into(),
    }
}

fn args(action: &StashAction) -> Vec<String> {
    let Plan::Git(commands) = action.plan(Path::new("/r")) else {
        panic!("expected git commands for {action:?}");
    };
    assert_eq!(commands.len(), 1);
    commands[0].argv()[6..].to_vec()
}

#[test]
fn apply_and_pop_optionally_reinstate_the_index() {
    let mut apply = StashAction::Apply {
        stash: stash(),
        reinstate_index: false,
    };
    assert_eq!(args(&apply), ["stash", "apply", "--", "stash@{1}"]);
    apply
        .set_input(InputId::ReinstateIndex, InputValue::Bool(true))
        .unwrap();
    assert_eq!(
        args(&apply),
        ["stash", "apply", "--index", "--", "stash@{1}"]
    );

    let pop = StashAction::Pop {
        stash: stash(),
        reinstate_index: true,
    };
    assert_eq!(args(&pop), ["stash", "pop", "--index", "--", "stash@{1}"]);
}

#[test]
fn drop_and_create_branch() {
    assert_eq!(
        args(&StashAction::Drop { stash: stash() }),
        ["stash", "drop", "--", "stash@{1}"]
    );
    assert_eq!(
        args(&StashAction::CreateBranch {
            stash: stash(),
            name: "rescued".into()
        }),
        ["stash", "branch", "--", "rescued", "stash@{1}"]
    );
}

#[test]
fn copy_name_is_the_selector() {
    assert_eq!(
        StashAction::CopyName { stash: stash() }.plan(Path::new("/r")),
        Plan::Clipboard("stash@{1}".into())
    );
}

#[test]
fn only_drop_is_destructive() {
    let drop = StashAction::Drop { stash: stash() };
    assert!(drop.is_destructive());
    assert_eq!(
        drop.confirmation().unwrap().message,
        "Drop stash@{1} \"On main: WIP\"? Its changes are deleted."
    );
    let pop = StashAction::Pop {
        stash: stash(),
        reinstate_index: false,
    };
    assert!(!pop.is_destructive());
    assert_eq!(pop.confirmation(), None);
}

#[test]
fn create_branch_validates_the_name() {
    let mut branch = StashAction::CreateBranch {
        stash: stash(),
        name: String::new(),
    };
    assert_eq!(branch.inputs()[0].id, InputId::Name);
    assert!(
        branch
            .set_input(InputId::Name, InputValue::Text("a..b".into()))
            .is_err()
    );
    branch
        .set_input(InputId::Name, InputValue::Text("rescued".into()))
        .unwrap();
}
