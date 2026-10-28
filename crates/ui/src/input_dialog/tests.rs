use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use tgg_actions::{Action, InputId, InputValue, ResetMode};
use tgg_model::Oid;

use super::*;
use crate::{Button, DialogStyle, Key, Theme};

fn oid() -> Oid {
    Oid::parse(&"a".repeat(40)).unwrap()
}

fn create_branch() -> InputDialog {
    InputDialog::for_action(
        "Create Branch",
        &Action::CreateBranch {
            at: oid(),
            name: String::new(),
            checkout: false,
        },
    )
    .unwrap()
}

fn type_text(dialog: &mut InputDialog, text: &str) {
    for ch in text.chars() {
        dialog.handle_key(Key::Char(ch));
    }
}

fn screen() -> Rect {
    Rect::new(0, 0, 80, 24)
}

fn render(dialog: &InputDialog) -> (Buffer, InputHitMap) {
    let mut buf = Buffer::empty(screen());
    let hits = dialog.render(screen(), &mut buf, &Theme::default());
    (buf, hits)
}

fn submitted(outcome: InputOutcome) -> Action {
    match outcome {
        InputOutcome::Submitted(action) => action,
        other => panic!("expected a submit, got {other:?}"),
    }
}

// ---- text editing ----

#[test]
fn text_field_edits_at_the_cursor() {
    let mut field = TextField::new("ac");
    assert_eq!(field.cursor(), 2, "starts at the end");
    field.left();
    field.insert('b');
    assert_eq!((field.text(), field.cursor()), ("abc", 2));
    field.home();
    field.delete();
    assert_eq!((field.text(), field.cursor()), ("bc", 0));
    field.end();
    field.backspace();
    assert_eq!((field.text(), field.cursor()), ("b", 1));
    field.right();
    assert_eq!(field.cursor(), 1, "can't move past the end");
    field.home();
    field.backspace();
    assert_eq!(
        (field.text(), field.cursor()),
        ("b", 0),
        "nothing before the start"
    );
    field.end();
    field.delete();
    assert_eq!(field.text(), "b", "nothing after the end");
}

#[test]
fn text_field_is_unicode_safe() {
    let mut field = TextField::new("añ日");
    field.backspace();
    assert_eq!(field.text(), "añ");
    field.left();
    field.insert('é');
    assert_eq!(field.text(), "aéñ");
    assert_eq!(field.cursor(), 2);
    field.end();
    field.insert('日');
    assert_eq!(field.width_before_cursor(), 5, "wide chars count twice");
}

// ---- focus ----

#[test]
fn focus_starts_on_the_first_field_and_tabs_through_fields_and_buttons() {
    let mut dialog = create_branch();
    assert_eq!(dialog.focus(), Focus::Field(0));
    dialog.handle_key(Key::Tab);
    assert_eq!(dialog.focus(), Focus::Field(1));
    dialog.handle_key(Key::Tab);
    assert_eq!(dialog.focus(), Focus::Button(Button::Confirm));
    dialog.handle_key(Key::Tab);
    assert_eq!(dialog.focus(), Focus::Button(Button::Cancel));
    dialog.handle_key(Key::Tab);
    assert_eq!(dialog.focus(), Focus::Field(0), "wraps");
    dialog.handle_key(Key::BackTab);
    assert_eq!(dialog.focus(), Focus::Button(Button::Cancel));
    dialog.handle_key(Key::Down);
    assert_eq!(dialog.focus(), Focus::Field(0), "Down moves like Tab");
    dialog.handle_key(Key::Up);
    assert_eq!(dialog.focus(), Focus::Button(Button::Cancel));
}

#[test]
fn toggles_and_choices_change_with_space_and_arrows() {
    let mut dialog = create_branch();
    dialog.handle_key(Key::Tab);
    dialog.handle_key(Key::Char(' '));
    assert_eq!(
        dialog.values()[1],
        (InputId::Checkout, InputValue::Bool(true))
    );

    let mut reset = InputDialog::for_action(
        "Reset",
        &Action::ResetCurrentTo {
            commit: oid(),
            subject: "s".into(),
            current: "main".into(),
            mode: ResetMode::Mixed,
        },
    )
    .unwrap();
    let mode = |d: &InputDialog| d.values()[0].1.clone();
    reset.handle_key(Key::Right);
    assert_eq!(mode(&reset), InputValue::Text("hard".into()));
    reset.handle_key(Key::Right);
    assert_eq!(mode(&reset), InputValue::Text("soft".into()), "wraps");
    reset.handle_key(Key::Left);
    assert_eq!(mode(&reset), InputValue::Text("hard".into()));
}

#[test]
fn remotes_turn_the_remote_field_into_a_choice() {
    let push = Action::PushBranch {
        branch: "topic".into(),
        remote: "origin".into(),
        set_upstream: true,
        force_with_lease: false,
    };
    let mut dialog = InputDialog::for_action("Push Branch", &push)
        .unwrap()
        .with_remotes(&["origin".to_owned(), "upstream".to_owned()]);
    dialog.handle_key(Key::Right);
    assert_eq!(
        dialog.values()[0],
        (InputId::Remote, InputValue::Text("upstream".into()))
    );
}

// ---- submit and validation ----

#[test]
fn enter_validates_and_shows_the_error_under_the_field() {
    let mut dialog = create_branch();
    assert_eq!(dialog.handle_key(Key::Enter), InputOutcome::Pending);
    assert_eq!(dialog.error(), Some((0, "The name can't be empty.")));
    type_text(&mut dialog, "bad name");
    assert_eq!(dialog.error(), None, "editing clears the error");
    dialog.handle_key(Key::Tab);
    assert_eq!(dialog.handle_key(Key::Enter), InputOutcome::Pending);
    assert_eq!(
        dialog.error(),
        Some((0, "The name can't contain spaces or control characters."))
    );
    assert_eq!(
        dialog.focus(),
        Focus::Field(0),
        "focus jumps to the bad field"
    );
}

#[test]
fn a_valid_submit_returns_the_action_with_the_values_applied() {
    let mut dialog = create_branch();
    type_text(&mut dialog, "feature/x");
    dialog.handle_key(Key::Tab);
    dialog.handle_key(Key::Char(' '));
    let action = submitted(dialog.handle_key(Key::Enter));
    assert_eq!(
        action,
        Action::CreateBranch {
            at: oid(),
            name: "feature/x".into(),
            checkout: true,
        }
    );
}

#[test]
fn buttons_submit_or_cancel() {
    let mut dialog = create_branch();
    type_text(&mut dialog, "x");
    assert_eq!(dialog.handle_key(Key::Esc), InputOutcome::Cancelled);
    dialog.handle_key(Key::BackTab);
    assert_eq!(dialog.focus(), Focus::Button(Button::Cancel));
    assert_eq!(dialog.handle_key(Key::Enter), InputOutcome::Cancelled);
    dialog.handle_key(Key::Left);
    assert_eq!(dialog.focus(), Focus::Button(Button::Confirm));
    submitted(dialog.handle_key(Key::Enter));
}

#[test]
fn errors_from_git_can_be_shown_on_a_field() {
    let mut dialog = create_branch();
    dialog.set_error(InputId::Name, "A branch named 'x' already exists.");
    assert_eq!(
        dialog.error(),
        Some((0, "A branch named 'x' already exists."))
    );
}

#[test]
fn actions_without_inputs_have_no_dialog() {
    assert!(
        InputDialog::for_action(
            "Checkout",
            &Action::CheckoutBranch {
                name: "main".into()
            }
        )
        .is_none()
    );
}

// ---- mouse and cursor ----

#[test]
fn clicks_focus_fields_flip_toggles_and_press_buttons() {
    let mut dialog = create_branch();
    let (_, hits) = render(&dialog);
    let toggle = hits.field(1).unwrap();
    assert_eq!(hits.hit(toggle.x, toggle.y), InputHit::Field(1));
    assert_eq!(
        dialog.handle_click(&hits, toggle.x, toggle.y),
        InputOutcome::Pending
    );
    assert_eq!(dialog.focus(), Focus::Field(1));
    assert_eq!(dialog.values()[1].1, InputValue::Bool(true));

    let name = hits.field(0).unwrap();
    dialog.handle_click(&hits, name.x, name.y);
    assert_eq!(dialog.focus(), Focus::Field(0));

    let cancel = hits.button(Button::Cancel).unwrap();
    assert_eq!(
        dialog.handle_click(&hits, cancel.x, cancel.y),
        InputOutcome::Cancelled
    );
    assert_eq!(
        dialog.handle_click(&hits, 0, 0),
        InputOutcome::Pending,
        "outside"
    );
    assert_eq!(hits.hit(0, 0), InputHit::Outside);
}

#[test]
fn the_hit_map_reports_the_text_cursor() {
    let mut dialog = create_branch();
    type_text(&mut dialog, "日x");
    let (_, hits) = render(&dialog);
    let field = hits.field(0).unwrap();
    assert_eq!(hits.cursor(), Some(Position::new(field.x + 3, field.y)));
    dialog.handle_key(Key::Tab);
    let (_, hits) = render(&dialog);
    assert_eq!(hits.cursor(), None, "no cursor on a toggle");
}

#[test]
fn long_text_scrolls_to_keep_the_cursor_visible() {
    let mut dialog = create_branch();
    type_text(&mut dialog, &"a".repeat(100));
    let (_, hits) = render(&dialog);
    let field = hits.field(0).unwrap();
    assert_eq!(
        hits.cursor(),
        Some(Position::new(field.right() - 1, field.y))
    );
}

#[test]
fn errors_are_drawn_in_the_warning_color() {
    let mut dialog = create_branch();
    dialog.handle_key(Key::Enter);
    let (buf, hits) = render(&dialog);
    let field = hits.field(0).unwrap();
    assert_eq!(
        buf[(field.x, field.y + 1)].fg,
        DialogStyle::default().warning
    );
}
