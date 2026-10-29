use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};

use super::*;
use crate::{Icons, Key, Theme};

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

fn toolbar() -> Toolbar {
    Toolbar::new(&names(&["main", "develop"]), true)
}

fn render(toolbar: &Toolbar, width: u16) -> (String, ToolbarHitMap) {
    render_with(toolbar, width, &Theme::default())
}

fn render_with(toolbar: &Toolbar, width: u16, theme: &Theme) -> (String, ToolbarHitMap) {
    let area = Rect::new(0, 3, width, 1);
    let mut buf = Buffer::empty(Rect::new(0, 0, width, 5));
    let hits = toolbar.render(area, &mut buf, theme);
    let line = (0..width)
        .map(|x| buf[(x, 3)].symbol())
        .collect::<String>()
        .trim_end()
        .to_owned();
    (line, hits)
}

#[test]
fn branch_filter_summary() {
    assert_eq!(filter_summary(&[]), "Show All");
    assert_eq!(filter_summary(&names(&["main"])), "main");
    assert_eq!(
        filter_summary(&names(&["main", "develop"])),
        "main, develop"
    );
    assert_eq!(filter_summary(&names(&["a", "b", "c"])), "3 selected");
}

#[test]
fn full_layout_on_a_wide_terminal() {
    let (line, _) = render(&toolbar(), 100);
    assert_eq!(
        line,
        " Branches: [main, develop ▾]  [x] Show Remote Branches                                [↻] [⇣] [...]"
    );
}

#[test]
fn compact_layout_on_a_narrow_terminal() {
    let (line, _) = render(&Toolbar::new(&[], false), 53);
    assert_eq!(line, " [Show All ▾]  [ ] Remotes             [↻] [⇣] [...]");
}

#[test]
fn every_button_is_in_the_hit_map() {
    let (_, hits) = render(&toolbar(), 100);
    for button in [
        ToolbarButton::BranchFilter,
        ToolbarButton::ShowRemotes,
        ToolbarButton::Refresh,
        ToolbarButton::Fetch,
        ToolbarButton::RepoMenu,
    ] {
        let rect = hits.button(button).unwrap();
        assert_eq!(hits.hit(rect.x, rect.y), Some(button), "{button:?}");
    }
    assert_eq!(hits.hit(0, 3), None, "the margin");
    assert_eq!(hits.hit(12, 0), None, "another row");
}

#[test]
fn clicks_activate_buttons_with_menus_opening_below_them() {
    let mut toolbar = toolbar();
    let (_, hits) = render(&toolbar, 100);
    let filter = hits.button(ToolbarButton::BranchFilter).unwrap();
    assert_eq!(
        toolbar.handle_click(&hits, filter.x + 1, filter.y),
        ToolbarOutcome::OpenBranchFilter(Position::new(filter.x, filter.y + 1))
    );
    let more = hits.button(ToolbarButton::RepoMenu).unwrap();
    assert_eq!(
        toolbar.handle_click(&hits, more.x, more.y),
        ToolbarOutcome::OpenRepoMenu(Position::new(more.x, more.y + 1))
    );
    let remotes = hits.button(ToolbarButton::ShowRemotes).unwrap();
    assert_eq!(
        toolbar.handle_click(&hits, remotes.x, remotes.y),
        ToolbarOutcome::ToggleRemotes
    );
    let refresh = hits.button(ToolbarButton::Refresh).unwrap();
    assert_eq!(
        toolbar.handle_click(&hits, refresh.x, refresh.y),
        ToolbarOutcome::Refresh
    );
    let fetch = hits.button(ToolbarButton::Fetch).unwrap();
    assert_eq!(
        toolbar.handle_click(&hits, fetch.x, fetch.y),
        ToolbarOutcome::Fetch
    );
    assert_eq!(toolbar.handle_click(&hits, 0, 3), ToolbarOutcome::Stay);
}

#[test]
fn keyboard_focus_moves_between_buttons_and_activates_them() {
    let mut toolbar = toolbar();
    let (_, hits) = render(&toolbar, 100);
    assert_eq!(toolbar.focus(), None);
    toolbar.set_focus(Some(ToolbarButton::BranchFilter));
    toolbar.handle_key(Key::Right, &hits);
    assert_eq!(toolbar.focus(), Some(ToolbarButton::ShowRemotes));
    assert_eq!(
        toolbar.handle_key(Key::Char(' '), &hits),
        ToolbarOutcome::ToggleRemotes
    );
    toolbar.handle_key(Key::Tab, &hits);
    toolbar.handle_key(Key::Tab, &hits);
    toolbar.handle_key(Key::Tab, &hits);
    assert_eq!(toolbar.focus(), Some(ToolbarButton::RepoMenu));
    toolbar.handle_key(Key::Right, &hits);
    assert_eq!(toolbar.focus(), Some(ToolbarButton::BranchFilter), "wraps");
    toolbar.handle_key(Key::Left, &hits);
    assert_eq!(toolbar.focus(), Some(ToolbarButton::RepoMenu));
    toolbar.handle_key(Key::BackTab, &hits);
    assert_eq!(toolbar.focus(), Some(ToolbarButton::Fetch));
    assert_eq!(toolbar.handle_key(Key::Enter, &hits), ToolbarOutcome::Fetch);
    assert_eq!(toolbar.handle_key(Key::Esc, &hits), ToolbarOutcome::Leave);
    assert_eq!(toolbar.focus(), None);
}

#[test]
fn keyboard_menus_open_below_the_button_of_the_last_render() {
    let mut toolbar = toolbar();
    let (_, hits) = render(&toolbar, 100);
    toolbar.set_focus(Some(ToolbarButton::RepoMenu));
    let more = hits.button(ToolbarButton::RepoMenu).unwrap();
    assert_eq!(
        toolbar.activate(&hits),
        ToolbarOutcome::OpenRepoMenu(Position::new(more.x, more.y + 1))
    );
}

#[test]
fn the_focused_button_is_highlighted() {
    let mut toolbar = toolbar();
    toolbar.set_focus(Some(ToolbarButton::Refresh));
    let area = Rect::new(0, 0, 100, 1);
    let mut buf = Buffer::empty(area);
    let hits = toolbar.render(area, &mut buf, &Theme::default());
    let refresh = hits.button(ToolbarButton::Refresh).unwrap();
    assert_eq!(buf[(refresh.x, 0)].bg, Theme::default().selected_bg);
    let fetch = hits.button(ToolbarButton::Fetch).unwrap();
    assert_ne!(buf[(fetch.x, 0)].bg, Theme::default().selected_bg);
}

#[test]
fn nerd_font_icons_replace_the_plain_buttons() {
    let theme = Theme {
        icons: Icons::Nerd,
        ..Theme::default()
    };
    let (line, hits) = render_with(&toolbar(), 100, &theme);
    // Codicons: refresh, cloud-download, ellipsis.
    assert!(
        line.ends_with(" \u{eb37}   \u{eac2}   \u{ea7c}"),
        "{line:?}"
    );
    assert!(!line.contains("[↻]"), "{line:?}");
    let refresh = hits.button(ToolbarButton::Refresh).unwrap();
    assert_eq!(refresh.width, 3, "the icon with a space on each side");
}

#[test]
fn plain_icons_are_the_default() {
    assert_eq!(Theme::default().icons, Icons::Unicode);
    let (line, _) = render(&toolbar(), 100);
    assert!(line.ends_with("[↻] [⇣] [...]"), "{line:?}");
}

#[test]
fn the_more_button_can_live_elsewhere() {
    let mut toolbar = toolbar().without_more();
    let (line, hits) = render(&toolbar, 100);
    assert!(line.ends_with("[↻] [⇣]"), "{line:?}");
    assert!(hits.button(ToolbarButton::RepoMenu).is_none());
    // Arrows skip the missing button: Fetch wraps around to the branch filter.
    toolbar.set_focus(Some(ToolbarButton::Fetch));
    toolbar.handle_key(Key::Right, &hits);
    assert_eq!(toolbar.focus(), Some(ToolbarButton::BranchFilter));
}
