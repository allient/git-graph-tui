use std::path::Path;

use pretty_assertions::assert_eq;
use tgg_git::CommandKind;

use super::*;
use crate::{Action, Plan};

fn oid(c: char) -> Oid {
    Oid::parse(&c.to_string().repeat(40)).unwrap()
}

/// Rebasing `main` onto `a`: commits `c` ("Add c") then `d` ("Add d").
fn rebase() -> InteractiveRebase {
    InteractiveRebase::new(
        oid('a'),
        "main".into(),
        vec![(oid('c'), "Add c".into()), (oid('d'), "Add d".into())],
    )
}

fn actions(rebase: &InteractiveRebase) -> Vec<TodoAction> {
    rebase.items.iter().map(|i| i.action).collect()
}

#[test]
fn every_commit_starts_as_pick_in_todo_order() {
    let rebase = rebase();
    assert_eq!(actions(&rebase), [TodoAction::Pick, TodoAction::Pick]);
    assert_eq!(
        rebase.todo(),
        format!("pick {} Add c\npick {} Add d\n", oid('c'), oid('d'))
    );
}

#[test]
fn the_todo_names_each_action_and_follows_the_order() {
    let mut rebase = rebase();
    rebase.move_item(1, Direction::Up);
    rebase.set_action(1, TodoAction::Squash).unwrap();
    assert_eq!(
        rebase.todo(),
        format!("pick {} Add d\nsquash {} Add c\n", oid('d'), oid('c'))
    );
    rebase.set_action(1, TodoAction::Fixup).unwrap();
    rebase.set_action(0, TodoAction::Drop).unwrap();
    assert_eq!(
        rebase.todo(),
        format!("drop {} Add d\nfixup {} Add c\n", oid('d'), oid('c'))
    );
}

#[test]
fn the_first_commit_cannot_be_squashed_or_fixed_up() {
    let mut rebase = rebase();
    for action in [TodoAction::Squash, TodoAction::Fixup] {
        assert_eq!(
            rebase.set_action(0, action),
            Err(RebaseError::NothingToSquashInto)
        );
    }
    assert_eq!(actions(&rebase), [TodoAction::Pick, TodoAction::Pick]);
    // Nor can one whose earlier commits are all dropped.
    rebase.set_action(0, TodoAction::Drop).unwrap();
    assert_eq!(
        rebase.set_action(1, TodoAction::Squash),
        Err(RebaseError::NothingToSquashInto)
    );
}

#[test]
fn cycling_skips_what_does_not_apply() {
    let mut rebase = rebase();
    let mut seen = Vec::new();
    for _ in 0..4 {
        rebase.cycle(1);
        seen.push(rebase.items[1].action);
    }
    assert_eq!(
        seen,
        [
            TodoAction::Squash,
            TodoAction::Fixup,
            TodoAction::Drop,
            TodoAction::Pick
        ]
    );
    rebase.cycle(0);
    assert_eq!(rebase.items[0].action, TodoAction::Drop);
    rebase.cycle(0);
    assert_eq!(rebase.items[0].action, TodoAction::Pick);
}

#[test]
fn moving_swaps_with_the_neighbour_and_stops_at_the_ends() {
    let mut rebase = rebase();
    assert_eq!(rebase.move_item(0, Direction::Up), 0);
    assert_eq!(rebase.move_item(0, Direction::Down), 1);
    assert_eq!(rebase.items[1].id, oid('c'));
    assert_eq!(rebase.move_item(1, Direction::Down), 1);
}

#[test]
fn the_preview_counts_the_commits_left() {
    let mut rebase = rebase();
    assert_eq!(rebase.result_len(), 2);
    rebase.set_action(1, TodoAction::Squash).unwrap();
    assert_eq!(rebase.result_len(), 1);
    rebase.set_action(1, TodoAction::Drop).unwrap();
    assert_eq!(rebase.result_len(), 1);
    assert_eq!(rebase.preview(), "2 commits → 1");
}

#[test]
fn validation_refuses_what_git_would_reject_or_nothing_at_all() {
    let mut rebase = rebase();
    assert_eq!(rebase.validate(), Err(RebaseError::Unchanged));
    rebase.set_action(1, TodoAction::Squash).unwrap();
    assert_eq!(rebase.validate(), Ok(()));
    // Squashing into a commit that is then dropped leaves nothing before it.
    rebase.set_action(0, TodoAction::Drop).unwrap();
    assert_eq!(rebase.validate(), Err(RebaseError::NothingToSquashInto));
    rebase.set_action(1, TodoAction::Drop).unwrap();
    assert_eq!(rebase.validate(), Err(RebaseError::NoneKept));
}

#[test]
fn a_reorder_alone_is_a_change() {
    let mut rebase = rebase();
    rebase.move_item(1, Direction::Up);
    assert_eq!(rebase.validate(), Ok(()));
}

#[test]
fn the_plan_replaces_the_todo_and_never_opens_an_editor() {
    let mut rebase = rebase();
    rebase.set_action(1, TodoAction::Drop).unwrap();
    let Plan::Git(commands) = Action::InteractiveRebase(rebase).plan(Path::new("/repo")) else {
        panic!("expected git commands");
    };
    assert_eq!(commands.len(), 1);
    let command = &commands[0];
    assert_eq!(command.kind(), CommandKind::Write);
    assert_eq!(
        command.argv()[6..],
        [
            "-c".to_owned(),
            "rebase.missingCommitsCheck=error".to_owned(),
            "rebase".to_owned(),
            "--interactive".to_owned(),
            "--no-autostash".to_owned(),
            oid('a').to_string()
        ]
    );
    let env = command.env();
    assert!(env.contains(&("GIT_EDITOR", "true")));
    let editor = env
        .iter()
        .find(|(k, _)| *k == "GIT_SEQUENCE_EDITOR")
        .map(|(_, v)| *v);
    assert_eq!(
        editor,
        Some(
            format!(
                "printf '%s\\n' 'pick {} Add c' 'drop {} Add d' >",
                oid('c'),
                oid('d')
            )
            .as_str()
        )
    );
}

#[test]
fn subjects_are_quoted_for_the_shell() {
    assert_eq!(shell_quote("it's $HOME `x`"), r"'it'\''s $HOME `x`'");
}

#[test]
fn it_confirms_naming_the_branch_and_the_reflog() {
    let action = Action::InteractiveRebase(rebase());
    assert!(action.is_destructive());
    assert!(action.conflicts_with_operation());
    let confirmation = action.confirmation().unwrap();
    assert_eq!(confirmation.title, "Interactive Rebase");
    assert_eq!(
        confirmation.message,
        "Rewrite 2 commits on 'main'? The old commits stay reachable in the reflog."
    );
    assert_eq!(confirmation.confirm_label, "Rebase");
}
