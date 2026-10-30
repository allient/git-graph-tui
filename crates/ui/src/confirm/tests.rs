use ggt_actions::{Action, Confirmation};
use ggt_model::Oid;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::*;
use crate::{Key, Theme};

fn oid() -> Oid {
    Oid::parse(&"a".repeat(40)).unwrap()
}

fn force_delete() -> Action {
    Action::DeleteBranch {
        name: "topic".into(),
        force: true,
    }
}

fn detached_checkout() -> Action {
    Action::CheckoutCommit {
        commit: oid(),
        subject: "Fix".into(),
    }
}

fn screen() -> Rect {
    Rect::new(0, 0, 80, 20)
}

fn render(dialog: &ConfirmDialog) -> (Buffer, DialogHitMap) {
    let mut buf = Buffer::empty(screen());
    let hits = dialog.render(screen(), &mut buf, &Theme::default());
    (buf, hits)
}

fn center(rect: Rect) -> (u16, u16) {
    (rect.x + rect.width / 2, rect.y)
}

#[test]
fn only_actions_with_a_confirmation_get_a_dialog() {
    assert!(ConfirmDialog::for_action(&force_delete()).is_some());
    assert!(
        ConfirmDialog::for_action(&Action::CheckoutBranch {
            name: "main".into()
        })
        .is_none()
    );
}

#[test]
fn destructive_dialogs_focus_cancel_and_others_focus_confirm() {
    let destructive = ConfirmDialog::for_action(&force_delete()).unwrap();
    assert!(destructive.is_destructive());
    assert_eq!(destructive.focus(), Button::Cancel);

    let safe = ConfirmDialog::for_action(&detached_checkout()).unwrap();
    assert!(!safe.is_destructive());
    assert_eq!(safe.focus(), Button::Confirm);
}

#[test]
fn keys_move_focus_and_answer() {
    let mut dialog = ConfirmDialog::for_action(&force_delete()).unwrap();
    assert_eq!(
        dialog.handle_key(Key::Enter),
        DialogOutcome::Cancelled,
        "Cancel has focus"
    );
    for key in [Key::Left, Key::Right, Key::Tab, Key::BackTab] {
        let before = dialog.focus();
        assert_eq!(dialog.handle_key(key), DialogOutcome::Pending);
        assert_ne!(dialog.focus(), before, "{key:?} moves focus");
    }
    dialog.handle_key(Key::Left);
    assert_eq!(dialog.focus(), Button::Confirm);
    assert_eq!(dialog.handle_key(Key::Enter), DialogOutcome::Confirmed);
    assert_eq!(dialog.handle_key(Key::Esc), DialogOutcome::Cancelled);
    assert_eq!(dialog.handle_key(Key::Char('x')), DialogOutcome::Pending);
}

#[test]
fn clicks_on_buttons_answer_and_clicks_elsewhere_do_nothing() {
    let mut dialog = ConfirmDialog::for_action(&detached_checkout()).unwrap();
    let (_, hits) = render(&dialog);
    let confirm = hits.button(Button::Confirm).unwrap();
    let cancel = hits.button(Button::Cancel).unwrap();
    let (x, y) = center(confirm);
    assert_eq!(hits.hit(x, y), DialogHit::Button(Button::Confirm));
    assert_eq!(dialog.handle_click(&hits, x, y), DialogOutcome::Confirmed);
    let (x, y) = center(cancel);
    assert_eq!(dialog.handle_click(&hits, x, y), DialogOutcome::Cancelled);
    let area = hits.area();
    assert_eq!(hits.hit(area.x, area.y), DialogHit::Frame);
    assert_eq!(
        dialog.handle_click(&hits, 0, 0),
        DialogOutcome::Pending,
        "outside"
    );
    assert_eq!(hits.hit(0, 0), DialogHit::Outside);
}

#[test]
fn dialog_is_centered_and_sized_to_its_text() {
    let dialog = ConfirmDialog::for_action(&detached_checkout()).unwrap();
    let area = dialog.area(screen());
    assert_eq!(area.width, MAX_WIDTH);
    assert_eq!(area.x, (80 - MAX_WIDTH) / 2);
    assert_eq!(area.y + area.height / 2, 10, "vertically centered");
}

#[test]
fn destructive_dialogs_are_drawn_as_warnings() {
    let style = DialogStyle::default();
    let theme = Theme::default();
    let (buf, hits) = render(&ConfirmDialog::for_action(&force_delete()).unwrap());
    let area = hits.area();
    assert_eq!(buf[(area.x, area.y)].fg, style.warning, "warning border");
    let confirm = hits.button(Button::Confirm).unwrap();
    assert_eq!(buf[(confirm.x + 1, confirm.y)].fg, style.warning);
    let cancel = hits.button(Button::Cancel).unwrap();
    assert_eq!(
        buf[(cancel.x + 1, cancel.y)].bg,
        theme.selected_bg,
        "focused"
    );

    let (buf, hits) = render(&ConfirmDialog::for_action(&detached_checkout()).unwrap());
    let area = hits.area();
    assert_ne!(buf[(area.x, area.y)].fg, style.warning);
}

#[test]
fn a_custom_confirmation_can_be_shown() {
    let dialog = ConfirmDialog::new(
        Confirmation {
            title: "T".into(),
            message: "M".into(),
            confirm_label: "Go".into(),
        },
        false,
    );
    assert_eq!(dialog.confirmation().confirm_label, "Go");
}

#[test]
fn wrap_breaks_on_spaces_and_splits_long_words() {
    assert_eq!(wrap("one two three", 7), ["one two", "three"]);
    assert_eq!(wrap("abcdefghij", 4), ["abcd", "efgh", "ij"]);
    assert_eq!(wrap("日本語テキスト", 6), ["日本語", "テキス", "ト"]);
    assert_eq!(wrap("", 5), Vec::<String>::new());
}
