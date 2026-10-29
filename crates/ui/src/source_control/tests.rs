use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use tgg_model::{Change, ChangedFile};

use super::*;
use crate::{Icons, Key, Theme};

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

fn render_with(
    pane: &mut SourceControl,
    height: u16,
    focused: bool,
    theme: &Theme,
) -> (Vec<String>, SourceControlHitMap) {
    let area = Rect::new(0, 0, W, height);
    let mut buf = Buffer::empty(area);
    let hits = pane.render(area, &mut buf, theme, focused);
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

fn render(
    pane: &mut SourceControl,
    height: u16,
    focused: bool,
) -> (Vec<String>, SourceControlHitMap) {
    render_with(pane, height, focused, &Theme::default())
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

/// Message → Commit button → the first list item.
const TO_LIST: [Key; 2] = [Key::Down, Key::Down];

#[test]
fn lists_staged_and_unstaged_files_like_vs_code() {
    let (lines, _) = render(&mut pane(), 13, false);
    assert_eq!(
        lines,
        [
            " SOURCE CONTROL               ✓ ↻ …",
            " Message (Enter commits on \"main\")",
            " Alt+Enter: new line",
            "",
            "              ✓ Commit",
            "",
            "▾ Staged Changes                − 2",
            "  app.rs src                    − M",
            "  README.md                     − M",
            "▾ Changes                     ↶ + 2",
            "  README.md                   ↶ + M",
            "  notes.txt                   ↶ + U",
            "",
        ]
    );
}

#[test]
fn nerd_font_icons_replace_the_buttons() {
    let theme = Theme {
        icons: Icons::Nerd,
        ..Theme::default()
    };
    let (lines, _) = render_with(&mut pane(), 13, false, &theme);
    assert!(
        lines[0].ends_with("\u{eab2} \u{eb37} \u{ea7c}"),
        "{:?}",
        lines[0]
    );
    assert!(
        lines[11].ends_with("\u{eae2} \u{ea60} U"),
        "{:?}",
        lines[11]
    );
}

#[test]
fn enter_commits_the_message_and_alt_enter_adds_a_line() {
    let mut pane = pane();
    pane.focus_message();
    type_text(&mut pane, "Fix login");
    // The app turns Alt+Enter (and Shift+Enter, Ctrl+J) into a newline character.
    press(&mut pane, &[Key::Char('\n')]);
    type_text(&mut pane, "Details");
    assert_eq!(
        press(&mut pane, &[Key::Enter]),
        SourceControlOutcome::Commit("Fix login\nDetails".into())
    );
    pane.clear_message();
    assert_eq!(pane.message(), "");
}

#[test]
fn the_message_wraps_and_shows_three_lines() {
    let mut pane = pane();
    pane.focus_message();
    type_text(
        &mut pane,
        "Subject\n\nA body line that is longer than the pane",
    );
    let (lines, hits) = render(&mut pane, 13, true);
    // The cursor's line (the wrapped end) stays visible.
    assert_eq!(lines[1], "");
    assert_eq!(lines[2], " A body line that is longer than th");
    assert_eq!(lines[3], " e pane");
    assert_eq!(hits.cursor(), Some(Position::new(7, 3)));
}

#[test]
fn up_and_down_move_between_message_lines_before_leaving_it() {
    let mut pane = pane();
    pane.focus_message();
    type_text(&mut pane, "ab\ncd");
    press(&mut pane, &[Key::Up]);
    type_text(&mut pane, "X");
    assert_eq!(pane.message(), "abX\ncd");
    press(&mut pane, &[Key::Down]);
    type_text(&mut pane, "Y");
    assert_eq!(pane.message(), "abX\ncdY");
    // Down from the last line goes to the Commit button.
    assert_eq!(
        press(&mut pane, &[Key::Down, Key::Enter]),
        SourceControlOutcome::Commit("abX\ncdY".into())
    );
}

#[test]
fn space_stages_an_unstaged_file_and_unstages_a_staged_one() {
    let mut pane = pane();
    pane.focus_message();
    press(&mut pane, &TO_LIST);
    assert_eq!(
        press(&mut pane, &[Key::Down, Key::Char(' ')]),
        SourceControlOutcome::Unstage(paths(&["src/app.rs"]))
    );
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
    press(&mut pane, &TO_LIST);
    assert_eq!(
        press(&mut pane, &[Key::Enter]),
        SourceControlOutcome::Unstage(paths(&["src/app.rs", "README.md"]))
    );
    assert_eq!(
        press(&mut pane, &[Key::Down, Key::Down, Key::Down, Key::Enter]),
        SourceControlOutcome::Stage(paths(&["README.md", "notes.txt"]))
    );
}

#[test]
fn d_discards_a_file_or_the_whole_changes_section() {
    let mut pane = pane();
    pane.focus_message();
    press(&mut pane, &TO_LIST);
    press(&mut pane, &[Key::End]);
    assert_eq!(
        press(&mut pane, &[Key::Char('d')]),
        SourceControlOutcome::Discard {
            paths: Vec::new(),
            untracked: paths(&["notes.txt"]),
        }
    );
    // On the Changes header: every unstaged file.
    press(&mut pane, &[Key::Up, Key::Up]);
    assert_eq!(
        press(&mut pane, &[Key::Delete]),
        SourceControlOutcome::Discard {
            paths: paths(&["README.md"]),
            untracked: paths(&["notes.txt"]),
        }
    );
    // Staged files have nothing to discard.
    press(&mut pane, &[Key::Home, Key::Down]);
    assert_eq!(
        press(&mut pane, &[Key::Char('d')]),
        SourceControlOutcome::Stay
    );
}

#[test]
fn plus_and_minus_keys_stage_and_unstage_the_selected_file() {
    let mut pane = pane();
    pane.focus_message();
    press(&mut pane, &TO_LIST);
    press(&mut pane, &[Key::End]);
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
    press(&mut pane, &TO_LIST);
    assert_eq!(
        press(&mut pane, &[Key::Down, Key::Char(' ')]),
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
fn header_buttons_commit_refresh_and_open_more_actions() {
    let mut pane = pane();
    let (_, hits) = render(&mut pane, 13, true);
    type_text(&mut pane, "");
    pane.focus_message();
    type_text(&mut pane, "Via the check");
    assert_eq!(
        pane.handle_click(&hits, 30, 0),
        SourceControlOutcome::Commit("Via the check".into())
    );
    assert_eq!(
        pane.handle_click(&hits, 32, 0),
        SourceControlOutcome::Refresh
    );
    assert_eq!(
        pane.handle_click(&hits, 34, 0),
        SourceControlOutcome::More(Position::new(34, 1))
    );
    // `m` opens the same menu from the keyboard.
    press(&mut pane, &[Key::Esc]);
    pane.focus_message();
    press(&mut pane, &TO_LIST);
    assert_eq!(
        press(&mut pane, &[Key::Char('m')]),
        SourceControlOutcome::More(Position::new(34, 1))
    );
}

#[test]
fn row_buttons_stage_unstage_and_discard() {
    let mut pane = pane();
    let (_, hits) = render(&mut pane, 13, true);
    // notes.txt is on line 11: ↶ at x 30, + at x 32.
    assert_eq!(
        pane.handle_click(&hits, 32, 11),
        SourceControlOutcome::Stage(paths(&["notes.txt"]))
    );
    assert_eq!(
        pane.handle_click(&hits, 30, 11),
        SourceControlOutcome::Discard {
            paths: Vec::new(),
            untracked: paths(&["notes.txt"]),
        }
    );
    // The Changes header's ↶ and + act on the whole section.
    assert_eq!(
        pane.handle_click(&hits, 32, 9),
        SourceControlOutcome::Stage(paths(&["README.md", "notes.txt"]))
    );
    // The − of the Staged Changes header unstages everything staged.
    assert_eq!(
        pane.handle_click(&hits, 32, 6),
        SourceControlOutcome::Unstage(paths(&["src/app.rs", "README.md"]))
    );
}

#[test]
fn clicks_focus_the_message_and_press_commit() {
    let mut pane = pane();
    let (_, hits) = render(&mut pane, 13, true);
    assert_eq!(pane.handle_click(&hits, 3, 2), SourceControlOutcome::Stay);
    type_text(&mut pane, "From a click");
    assert_eq!(
        pane.handle_click(&hits, 15, 4),
        SourceControlOutcome::Commit("From a click".into())
    );
}

#[test]
fn the_cursor_shows_in_the_focused_message() {
    let mut pane = pane();
    pane.focus_message();
    type_text(&mut pane, "ab");
    let (lines, hits) = render(&mut pane, 13, true);
    assert_eq!(lines[1], " ab");
    assert_eq!(hits.cursor(), Some(Position::new(3, 1)));
    let (_, unfocused) = render(&mut pane, 13, false);
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
    press(&mut pane, &TO_LIST);
    press(&mut pane, &[Key::End]);
    let (lines, _) = render(&mut pane, 12, true);
    assert_eq!(lines[11], "  f19.txt                     ↶ + U");
}

#[test]
fn new_changes_keep_the_selection_in_range() {
    let mut pane = pane();
    pane.focus_message();
    press(&mut pane, &TO_LIST);
    press(&mut pane, &[Key::End]);
    pane.set_changes(vec![file("a.txt", None, Some(Change::Untracked))], None);
    assert_eq!(
        press(&mut pane, &[Key::Char(' ')]),
        SourceControlOutcome::Stage(paths(&["a.txt"]))
    );
}
