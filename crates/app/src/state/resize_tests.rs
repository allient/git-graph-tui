//! Resizing the Source Control pane and the graph's columns by dragging their
//! dividers (S6).

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use pretty_assertions::assert_eq;
use tgg_testkit::FixtureRepo;

use super::*;
use crate::testing::Harness;

const H: u16 = 24;

fn open(repo: &FixtureRepo, width: u16) -> Harness {
    let mut h = Harness::open(repo);
    h.draw(width, H);
    h
}

fn mouse(h: &mut Harness, kind: MouseEventKind, x: u16, y: u16, width: u16) {
    let commands = h.state.update(Event::Mouse(MouseEvent {
        kind,
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }));
    h.settle(commands);
    h.draw(width, H);
}

fn drag(h: &mut Harness, from: (u16, u16), to_x: u16, width: u16) {
    let left = MouseButton::Left;
    mouse(h, MouseEventKind::Down(left), from.0, from.1, width);
    mouse(h, MouseEventKind::Drag(left), to_x, from.1, width);
    mouse(h, MouseEventKind::Up(left), to_x, from.1, width);
}

fn find(h: &Harness, text: &str) -> (u16, u16) {
    h.find_on_screen(text)
        .unwrap_or_else(|| panic!("{text:?} not on screen"))
}

fn cell(h: &Harness, x: u16, y: u16) -> &str {
    h.screen[(x, y)].symbol()
}

#[test]
fn dragging_the_divider_between_the_panes_resizes_source_control() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo, 110);
    // A quarter of 110 is under the 30-column minimum default.
    assert_eq!(cell(&h, 30, 5), "│");
    assert_eq!(find(&h, "Graph").0, 31);

    drag(&mut h, (30, 5), 40, 110);
    assert_eq!(cell(&h, 40, 5), "│");
    assert_eq!(find(&h, "Graph").0, 41);
    assert!(h.state.source_control.width.is_some());

    // The pane keeps at least 20 columns and the graph at least 50.
    drag(&mut h, (40, 5), 3, 110);
    assert_eq!(cell(&h, 20, 5), "│");
    drag(&mut h, (20, 5), 105, 110);
    assert_eq!(cell(&h, 60, 5), "│");
}

#[test]
fn dragging_a_header_divider_resizes_the_columns() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo, 150);
    let selected = h.state.selected;
    let (x, y) = find(&h, "│Date");
    let (author_x, _) = find(&h, "│Author");

    drag(&mut h, (x, y), x - 4, 150);
    assert_eq!(find(&h, "│Date"), (x - 4, y), "the date column is 4 wider");
    assert_eq!(
        find(&h, "│Author").0,
        author_x,
        "the columns on the right stay"
    );
    assert_eq!(h.state.selected, selected, "a drag doesn't select a row");
    assert!(h.state.details.is_none(), "nor open details");

    let (graph_x, _) = find(&h, "│Description");
    drag(&mut h, (graph_x, y), graph_x + 6, 150);
    assert_eq!(
        find(&h, "│Description").0,
        graph_x + 6,
        "a wider graph column"
    );
}

#[test]
fn moving_the_mouse_without_grabbing_a_divider_resizes_nothing() {
    let repo = FixtureRepo::sample();
    let mut h = open(&repo, 110);
    mouse(&mut h, MouseEventKind::Drag(MouseButton::Left), 50, 5, 110);
    assert_eq!(cell(&h, 30, 5), "│");
    assert!(h.state.source_control.width.is_none());
}

#[test]
fn a_saved_layout_is_applied_and_read_back() {
    use crate::config::Layout;
    let repo = FixtureRepo::sample();
    let mut h = Harness::open(&repo);
    let saved = Layout {
        source_control_width: Some(40),
        date_column: Some(20),
        ..Layout::default()
    };
    h.state.apply_layout(saved.clone());
    h.draw(150, H);
    assert_eq!(cell(&h, 40, 5), "│", "the saved pane width");
    assert_eq!(
        h.state.layout(),
        saved,
        "unchanged until something is dragged"
    );

    let (x, y) = find(&h, "│Description");
    drag(&mut h, (x, y), x + 3, 150);
    let layout = h.state.layout();
    assert_eq!(layout.source_control_width, Some(40));
    assert_eq!(layout.date_column, Some(20));
    assert!(
        layout.graph_column.is_some(),
        "the dragged graph column is saved"
    );
}

#[test]
fn default_widths_written_in_the_config_are_not_a_change() {
    use crate::config::{ChangesSortName, ChangesViewName, Layout};
    let repo = FixtureRepo::sample();
    let mut h = Harness::open(&repo);
    // The README's example spells out the default column widths.
    let example = Layout {
        source_control_width: Some(36),
        graph_column: Some(12),
        date_column: Some(16),
        author_column: Some(14),
        hash_column: Some(8),
        hidden_columns: Vec::new(),
        changes_view: Some(ChangesViewName::List),
        changes_sort: Some(ChangesSortName::Path),
    };
    let baseline = h.state.apply_layout(example);
    h.draw(150, H);
    assert_eq!(
        h.state.layout(),
        baseline,
        "quitting without resizing leaves the file alone"
    );
}

fn header_menu_labels(h: &Harness) -> Vec<String> {
    h.state
        .menu
        .as_ref()
        .map(|m| {
            m.items()
                .iter()
                .filter_map(|item| match item {
                    MenuItem::Entry(e) => Some(e.label.clone()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

fn right_click_header(h: &mut Harness, width: u16) {
    let (x, y) = find(h, "Description");
    mouse(h, MouseEventKind::Down(MouseButton::Right), x + 3, y, width);
}

/// Clicks the open menu's entry `label`.
fn choose(h: &mut Harness, label: &str, width: u16) {
    let (x, y) = find(h, label);
    mouse(h, MouseEventKind::Down(MouseButton::Left), x + 3, y, width);
}

#[test]
fn right_clicking_the_header_shows_or_hides_columns() {
    use crate::config::ColumnName;
    let repo = FixtureRepo::sample();
    let mut h = open(&repo, 150);
    right_click_header(&mut h, 150);
    assert_eq!(header_menu_labels(&h), ["✓ Date", "✓ Author", "✓ Commit"]);
    choose(&mut h, "✓ Author", 150);

    assert!(h.state.menu.is_none());
    assert!(
        h.find_on_screen("Author").is_none(),
        "the header lost Author"
    );
    assert!(
        h.find_on_screen("Fixture Author").is_none(),
        "the rows lost it too"
    );
    assert!(h.find_on_screen("│Date").is_some());
    assert!(h.find_on_screen("│Author").is_none(), "and its divider");
    assert!(
        h.find_on_screen("│Commit").is_some(),
        "commit now follows date"
    );
    assert_eq!(h.state.layout().hidden_columns, [ColumnName::Author]);

    // The menu now shows it unchecked; choosing it again brings it back.
    right_click_header(&mut h, 150);
    assert_eq!(header_menu_labels(&h), ["✓ Date", "  Author", "✓ Commit"]);
    choose(&mut h, "  Author", 150);
    assert!(h.find_on_screen("│Author").is_some());
    assert!(h.state.layout().hidden_columns.is_empty());
}

#[test]
fn hidden_columns_are_saved_and_applied() {
    use crate::config::{ColumnName, Layout};
    let repo = FixtureRepo::sample();
    let mut h = Harness::open(&repo);
    let saved = Layout {
        hidden_columns: vec![ColumnName::Date, ColumnName::Commit],
        ..Layout::default()
    };
    let baseline = h.state.apply_layout(saved.clone());
    assert_eq!(baseline, saved);
    h.draw(150, H);
    assert!(h.find_on_screen("│Date").is_none());
    assert!(h.find_on_screen("│Commit").is_none());
    assert!(h.find_on_screen("│Author").is_some());

    right_click_header(&mut h, 150);
    choose(&mut h, "  Date", 150);
    assert!(h.find_on_screen("│Date").is_some());
    assert_eq!(h.state.layout().hidden_columns, [ColumnName::Commit]);
}

#[test]
fn source_control_view_and_sort_are_saved_and_applied() {
    use crate::config::{ChangesSortName, ChangesViewName, Layout};
    use tgg_actions::{ChangesSort, ChangesView};
    let repo = FixtureRepo::sample();
    let mut h = Harness::open(&repo);
    // The defaults (list, by path) aren't written.
    assert_eq!(h.state.layout(), Layout::default());

    let saved = Layout {
        changes_view: Some(ChangesViewName::Tree),
        changes_sort: Some(ChangesSortName::Status),
        ..Layout::default()
    };
    assert_eq!(h.state.apply_layout(saved.clone()), saved);
    let applied = h.state.source_control.view.layout();
    assert_eq!(applied.view, ChangesView::Tree);
    assert_eq!(applied.sort, ChangesSort::Status);

    h.state.source_control.view.set_view(ChangesView::List);
    assert_eq!(h.state.layout().changes_view, None);
    assert_eq!(h.state.layout().changes_sort, Some(ChangesSortName::Status));
}
