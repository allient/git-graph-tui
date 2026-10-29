use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tgg_model::{Change, ChangedFile};

use super::*;
use crate::{Key, Theme};

const W: u16 = 36;

fn file(path: &str, staged: Option<Change>, unstaged: Option<Change>) -> ChangedFile {
    ChangedFile {
        path: path.to_owned(),
        orig_path: None,
        staged,
        unstaged,
    }
}

/// `src/app.rs` staged, `README.md` both staged and changed, `notes.txt` untracked.
fn pane() -> SourceControl {
    let mut pane = SourceControl::default();
    pane.set_changes(
        vec![
            file("src/app.rs", Some(Change::Modified), None),
            file("README.md", Some(Change::Modified), Some(Change::Modified)),
            file("notes.txt", None, Some(Change::Untracked)),
        ],
        Some("main".into()),
    );
    pane
}

fn render(
    pane: &mut SourceControl,
    height: u16,
    focused: bool,
) -> (Vec<String>, SourceControlHitMap) {
    let area = Rect::new(0, 0, W, height);
    let mut buf = Buffer::empty(area);
    let hits = pane.render(area, &mut buf, &Theme::default(), focused);
    let lines = (0..height)
        .map(|y| {
            (0..W)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect();
    (lines, hits)
}

fn press(pane: &mut SourceControl, keys: &[Key]) -> SourceControlOutcome {
    let mut last = SourceControlOutcome::Stay;
    for key in keys {
        last = pane.handle_key(*key);
    }
    last
}

fn type_text(pane: &mut SourceControl, text: &str) {
    for ch in text.chars() {
        pane.handle_key(Key::Char(ch));
    }
}

fn paths(list: &[&str]) -> Vec<String> {
    list.iter().map(|p| (*p).to_owned()).collect()
}

#[test]
fn lists_staged_and_unstaged_files_like_vs_code() {
    let (lines, _) = render(&mut pane(), 12, false);
    assert_eq!(
        lines,
        [
            " SOURCE CONTROL",
            " Message (Enter to commit on \"main\")",
            "              ✓ Commit",
            "",
            "▾ Staged Changes                 2 −",
            "  app.rs src                     M −",
            "  README.md                      M −",
            "▾ Changes                        2 +",
            "  README.md                      M +",
            "  notes.txt                      U +",
            "",
            "",
        ]
    );
}

#[test]
fn enter_in_the_message_commits_it() {
    let mut pane = pane();
    pane.focus_message();
    type_text(&mut pane, "Fix login");
    assert_eq!(pane.message(), "Fix login");
    assert_eq!(
        press(&mut pane, &[Key::Enter]),
        SourceControlOutcome::Commit("Fix login".into())
    );
    pane.clear_message();
    assert_eq!(pane.message(), "");
}

#[test]
fn the_commit_button_commits_too() {
    let mut pane = pane();
    pane.focus_message();
    type_text(&mut pane, "x");
    assert_eq!(
        press(&mut pane, &[Key::Down, Key::Enter]),
        SourceControlOutcome::Commit("x".into())
    );
}

#[test]
fn space_stages_an_unstaged_file_and_unstages_a_staged_one() {
    let mut pane = pane();
    pane.focus_message();
    // Message → Commit → first item (the Staged Changes header) → src/app.rs.
    assert_eq!(
        press(
            &mut pane,
            &[Key::Down, Key::Down, Key::Down, Key::Char(' ')]
        ),
        SourceControlOutcome::Unstage(paths(&["src/app.rs"]))
    );
    // Down past README.md (staged) and the Changes header to README.md (changed).
    assert_eq!(
        press(
            &mut pane,
            &[Key::Down, Key::Down, Key::Down, Key::Char(' ')]
        ),
        SourceControlOutcome::Stage(paths(&["README.md"]))
    );
}

#[test]
fn headers_stage_or_unstage_everything_in_their_section() {
    let mut pane = pane();
    pane.focus_message();
    assert_eq!(
        press(&mut pane, &[Key::Down, Key::Down, Key::Enter]),
        SourceControlOutcome::Unstage(paths(&["src/app.rs", "README.md"]))
    );
    assert_eq!(
        press(&mut pane, &[Key::Down, Key::Down, Key::Down, Key::Enter]),
        SourceControlOutcome::Stage(paths(&["README.md", "notes.txt"]))
    );
}

#[test]
fn plus_and_minus_keys_stage_and_unstage_the_selected_file() {
    let mut pane = pane();
    pane.focus_message();
    press(&mut pane, &[Key::Down, Key::Down, Key::End]);
    assert_eq!(
        press(&mut pane, &[Key::Char('+')]),
        SourceControlOutcome::Stage(paths(&["notes.txt"]))
    );
    assert_eq!(
        press(&mut pane, &[Key::Char('-')]),
        SourceControlOutcome::Stay
    );
}

#[test]
fn unstaging_a_rename_resets_both_paths() {
    let mut pane = SourceControl::default();
    pane.set_changes(
        vec![ChangedFile {
            path: "new.rs".into(),
            orig_path: Some("old.rs".into()),
            staged: Some(Change::Renamed),
            unstaged: None,
        }],
        None,
    );
    pane.focus_message();
    assert_eq!(
        press(
            &mut pane,
            &[Key::Down, Key::Down, Key::Down, Key::Char(' ')]
        ),
        SourceControlOutcome::Unstage(paths(&["new.rs", "old.rs"]))
    );
}

#[test]
fn escape_leaves_the_pane() {
    let mut pane = pane();
    pane.focus_message();
    assert_eq!(press(&mut pane, &[Key::Esc]), SourceControlOutcome::Leave);
}

#[test]
fn clicks_press_buttons_and_focus_the_message() {
    let mut pane = pane();
    let (lines, hits) = render(&mut pane, 12, true);
    // The `+` of notes.txt (line 9) stages it.
    let plus = u16::try_from(lines[9].chars().count() - 1).unwrap();
    assert_eq!(
        pane.handle_click(&hits, plus, 9),
        SourceControlOutcome::Stage(paths(&["notes.txt"]))
    );
    // The `−` of the Staged Changes header unstages everything staged.
    assert_eq!(
        pane.handle_click(&hits, W - 1, 4),
        SourceControlOutcome::Unstage(paths(&["src/app.rs", "README.md"]))
    );
    assert_eq!(pane.handle_click(&hits, 3, 1), SourceControlOutcome::Stay);
    type_text(&mut pane, "From a click");
    assert_eq!(
        pane.handle_click(&hits, 15, 2),
        SourceControlOutcome::Commit("From a click".into())
    );
}

#[test]
fn the_cursor_shows_in_the_focused_message() {
    let mut pane = pane();
    pane.focus_message();
    type_text(&mut pane, "ab");
    let (lines, hits) = render(&mut pane, 12, true);
    assert_eq!(lines[1], " ab");
    assert_eq!(hits.cursor(), Some(Position::new(3, 1)));
    let (_, unfocused) = render(&mut pane, 12, false);
    assert_eq!(unfocused.cursor(), None);
}

#[test]
fn a_long_list_scrolls_to_keep_the_selection_visible() {
    let mut pane = SourceControl::default();
    let many = (0..20)
        .map(|i| file(&format!("f{i:02}.txt"), None, Some(Change::Untracked)))
        .collect();
    pane.set_changes(many, None);
    pane.focus_message();
    press(&mut pane, &[Key::Down, Key::Down, Key::End]);
    let (lines, _) = render(&mut pane, 10, true);
    assert_eq!(lines[9], "  f19.txt                        U +");
}

#[test]
fn new_changes_keep_the_selection_in_range() {
    let mut pane = pane();
    pane.focus_message();
    press(&mut pane, &[Key::Down, Key::Down, Key::End]);
    pane.set_changes(vec![file("a.txt", None, Some(Change::Untracked))], None);
    assert_eq!(
        press(&mut pane, &[Key::Char(' ')]),
        SourceControlOutcome::Stage(paths(&["a.txt"]))
    );
}
