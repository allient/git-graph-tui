//! Renders S13 confirmation dialogs for real actions into a buffer.

#![expect(clippy::unwrap_used, reason = "test helpers")]

use ggt_actions::{Action, ResetMode};
use ggt_model::Oid;
use ggt_ui::{ConfirmDialog, Theme};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

fn render(action: &Action, width: u16, height: u16) -> String {
    let dialog = ConfirmDialog::for_action(action).unwrap();
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    for y in 0..height {
        buf.set_string(
            0,
            y,
            ".".repeat(usize::from(width)),
            ratatui::style::Style::default(),
        );
    }
    dialog.render(area, &mut buf, &Theme::default());
    (0..height)
        .map(|y| (0..width).map(|x| buf[(x, y)].symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn commit() -> Oid {
    Oid::parse("0123456789abcdef0123456789abcdef01234567").unwrap()
}

#[test]
fn force_delete_branch() {
    let action = Action::DeleteBranch {
        name: "feature/login".into(),
        force: true,
    };
    insta::assert_snapshot!(render(&action, 70, 12));
}

#[test]
fn hard_reset() {
    let action = Action::ResetCurrentTo {
        commit: commit(),
        subject: "Add lib".into(),
        current: "main".into(),
        mode: ResetMode::Hard,
    };
    insta::assert_snapshot!(render(&action, 70, 12));
}

#[test]
fn detached_checkout_on_a_narrow_terminal() {
    let action = Action::CheckoutCommit {
        commit: commit(),
        subject: "Fix the login page".into(),
    };
    insta::assert_snapshot!(render(&action, 34, 14));
}
