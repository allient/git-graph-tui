//! Renders input dialogs for real actions into a buffer.

#![expect(clippy::unwrap_used, reason = "test helpers")]

use ggt_actions::{Action, ResetMode};
use ggt_model::Oid;
use ggt_ui::{InputDialog, Key, Theme};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

fn render(dialog: &InputDialog, width: u16, height: u16) -> String {
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    dialog.render(area, &mut buf, &Theme::default());
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn commit() -> Oid {
    Oid::parse("0123456789abcdef0123456789abcdef01234567").unwrap()
}

#[test]
fn create_branch_with_a_validation_error() {
    let action = Action::CreateBranch {
        at: commit(),
        name: String::new(),
        checkout: false,
    };
    let mut dialog = InputDialog::for_action("Create Branch", &action).unwrap();
    for ch in "my branch".chars() {
        dialog.handle_key(Key::Char(ch));
    }
    dialog.handle_key(Key::Enter);
    insta::assert_snapshot!(render(&dialog, 64, 11));
}

#[test]
fn annotated_tag() {
    let action = Action::AddTag {
        at: commit(),
        name: "v1.0.0".into(),
        annotated: true,
        message: "First release".into(),
        remote: Some("origin".into()),
        push: false,
    };
    let dialog = InputDialog::for_action("Add Tag", &action).unwrap();
    insta::assert_snapshot!(render(&dialog, 64, 17));
}

#[test]
fn reset_mode_choice() {
    let action = Action::ResetCurrentTo {
        commit: commit(),
        subject: "Add lib".into(),
        current: "main".into(),
        mode: ResetMode::Mixed,
    };
    let dialog = InputDialog::for_action("Reset", &action).unwrap();
    insta::assert_snapshot!(render(&dialog, 64, 10));
}

#[test]
fn push_with_remote_choice_and_toggles() {
    let action = Action::PushBranch {
        branch: "feature/login".into(),
        remote: "origin".into(),
        set_upstream: true,
        force_with_lease: false,
    };
    let dialog = InputDialog::for_action("Push Branch", &action)
        .unwrap()
        .with_remotes(&["origin".to_owned(), "upstream".to_owned()]);
    insta::assert_snapshot!(render(&dialog, 64, 12));
}
