use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use tgg_actions::{Action, MenuEntry, MenuItem};

use super::*;
use crate::{Key, Theme};

fn entry(label: &str) -> MenuItem {
    MenuItem::Entry(MenuEntry::new(
        label,
        Action::CopyBranchName { name: label.into() },
    ))
}

fn disabled(label: &str, why: &str) -> MenuItem {
    MenuItem::Entry(
        MenuEntry::new(label, Action::CopyBranchName { name: label.into() })
            .disabled_because(Some(why)),
    )
}

/// `Checkout`, `---`, `Drop` (disabled), `Delete`, `---`, `Copy`.
fn menu() -> PopupMenu {
    PopupMenu::new(
        vec![
            entry("Checkout"),
            MenuItem::Separator,
            disabled("Drop...", "Merge commits can't be dropped."),
            entry("Delete..."),
            MenuItem::Separator,
            entry("Copy"),
        ],
        Position::new(0, 0),
    )
}

fn screen() -> Rect {
    Rect::new(0, 0, 40, 12)
}

fn chosen_label(outcome: MenuOutcome) -> String {
    match outcome {
        MenuOutcome::Chosen(Action::CopyBranchName { name }) => name,
        other => panic!("expected a chosen entry, got {other:?}"),
    }
}

// ---- selection ----

#[test]
fn opens_on_the_first_enabled_entry() {
    let menu = PopupMenu::new(
        vec![disabled("Drop...", "no"), entry("Copy")],
        Position::new(0, 0),
    );
    assert_eq!(menu.selected(), Some(1));
}

#[test]
fn next_and_prev_skip_separators_and_disabled_entries_and_wrap() {
    let mut menu = menu();
    assert_eq!(menu.selected(), Some(0));
    menu.select_next();
    assert_eq!(menu.selected(), Some(3), "skips the separator and Drop");
    menu.select_next();
    assert_eq!(menu.selected(), Some(5));
    menu.select_next();
    assert_eq!(menu.selected(), Some(0), "wraps to the top");
    menu.select_prev();
    assert_eq!(menu.selected(), Some(5), "wraps to the bottom");
    menu.select_first();
    assert_eq!(menu.selected(), Some(0));
    menu.select_last();
    assert_eq!(menu.selected(), Some(5));
}

#[test]
fn a_menu_without_enabled_entries_selects_nothing() {
    let mut menu = PopupMenu::new(vec![disabled("Drop...", "no")], Position::new(0, 0));
    assert_eq!(menu.selected(), None);
    menu.select_next();
    assert_eq!(menu.selected(), None);
    assert_eq!(menu.handle_key(Key::Enter), MenuOutcome::Open);
}

#[test]
fn accelerator_letter_jumps_to_the_next_matching_enabled_entry() {
    let mut menu = PopupMenu::new(
        vec![
            entry("Checkout"),
            entry("Cherry Pick..."),
            disabled("Create Branch...", "no"),
            entry("Copy"),
        ],
        Position::new(0, 0),
    );
    assert_eq!(menu.handle_key(Key::Char('c')), MenuOutcome::Open);
    assert_eq!(menu.selected(), Some(1));
    menu.handle_key(Key::Char('C'));
    assert_eq!(menu.selected(), Some(3), "case-insensitive, skips disabled");
    menu.handle_key(Key::Char('c'));
    assert_eq!(menu.selected(), Some(0), "wraps");
    menu.handle_key(Key::Char('z'));
    assert_eq!(menu.selected(), Some(0), "no match keeps the selection");
}

#[test]
fn keys_choose_or_close() {
    let mut menu = menu();
    menu.handle_key(Key::Down);
    assert_eq!(chosen_label(menu.handle_key(Key::Enter)), "Delete...");
    assert_eq!(menu.handle_key(Key::Esc), MenuOutcome::Closed);
    menu.handle_key(Key::End);
    assert_eq!(menu.selected(), Some(5));
    menu.handle_key(Key::Home);
    assert_eq!(menu.selected(), Some(0));
    menu.handle_key(Key::Up);
    assert_eq!(menu.selected(), Some(5));
}

#[test]
fn disabled_entries_expose_their_reason() {
    let menu = menu();
    assert_eq!(
        menu.disabled_reason(2),
        Some("Merge commits can't be dropped.")
    );
    assert_eq!(menu.disabled_reason(0), None);
    assert_eq!(menu.disabled_reason(99), None);
}

// ---- placement ----

#[test]
fn size_fits_the_longest_label_plus_border_and_padding() {
    // "Delete..." is 9 columns: 9 + 2 padding + 2 border.
    assert_eq!(menu().area(screen()), Rect::new(0, 0, 13, 8));
}

#[test]
fn opens_at_the_anchor_when_there_is_room() {
    let menu = PopupMenu::new(menu().items().to_vec(), Position::new(5, 2));
    assert_eq!(menu.area(screen()), Rect::new(5, 2, 13, 8));
}

#[test]
fn flips_left_and_up_near_the_edges() {
    let menu = PopupMenu::new(menu().items().to_vec(), Position::new(35, 9));
    // Right edge at the anchor column, bottom edge on the anchor row.
    assert_eq!(menu.area(screen()), Rect::new(23, 2, 13, 8));
}

#[test]
fn stays_inside_a_screen_smaller_than_the_menu() {
    let tiny = Rect::new(2, 1, 10, 5);
    let menu = PopupMenu::new(menu().items().to_vec(), Position::new(8, 3));
    assert_eq!(menu.area(tiny), tiny);
}

// ---- rendering and hit-testing ----

fn render(menu: &PopupMenu, area: Rect) -> (Buffer, MenuHitMap) {
    let mut buf = Buffer::empty(area);
    let hits = menu.render(area, &mut buf, &Theme::default());
    (buf, hits)
}

#[test]
fn hit_map_resolves_items_frame_and_outside() {
    let menu = PopupMenu::new(menu().items().to_vec(), Position::new(2, 1));
    let (_, hits) = render(&menu, screen());
    // Item rows start inside the top border.
    assert_eq!(hits.hit(4, 2), MenuHit::Item(0));
    assert_eq!(hits.hit(4, 3), MenuHit::Frame, "separator");
    assert_eq!(hits.hit(4, 4), MenuHit::Item(2));
    assert_eq!(hits.hit(2, 1), MenuHit::Frame, "border");
    assert_eq!(hits.hit(30, 10), MenuHit::Outside);
}

#[test]
fn clicks_choose_enabled_items_and_explain_disabled_ones() {
    let mut menu = PopupMenu::new(menu().items().to_vec(), Position::new(2, 1));
    let (_, hits) = render(&menu, screen());
    assert_eq!(
        menu.handle_click(&hits, 4, 4),
        MenuOutcome::Disabled("Merge commits can't be dropped.".into())
    );
    assert_eq!(menu.handle_click(&hits, 4, 3), MenuOutcome::Open);
    assert_eq!(menu.handle_click(&hits, 30, 10), MenuOutcome::Closed);
    assert_eq!(chosen_label(menu.handle_click(&hits, 4, 5)), "Delete...");
}

#[test]
fn hover_moves_the_selection_to_enabled_items_only() {
    let mut menu = PopupMenu::new(menu().items().to_vec(), Position::new(2, 1));
    let (_, hits) = render(&menu, screen());
    menu.handle_hover(&hits, 4, 5);
    assert_eq!(menu.selected(), Some(3));
    menu.handle_hover(&hits, 4, 4);
    assert_eq!(
        menu.selected(),
        Some(3),
        "disabled entry keeps the selection"
    );
}

#[test]
fn selected_entry_is_highlighted_and_disabled_entries_are_dimmed() {
    let theme = Theme::default();
    let menu = PopupMenu::new(menu().items().to_vec(), Position::new(0, 0));
    let (buf, _) = render(&menu, screen());
    assert_eq!(buf[(2, 1)].bg, theme.selected_bg, "Checkout is selected");
    assert_eq!(buf[(2, 3)].fg, theme.muted, "Drop... is dimmed");
    assert_eq!(buf[(2, 4)].fg, theme.text);
}

#[test]
fn long_menus_scroll_to_keep_the_selection_visible() {
    let items: Vec<MenuItem> = (0..10).map(|i| entry(&format!("Item {i}"))).collect();
    let mut menu = PopupMenu::new(items, Position::new(0, 0));
    let short = Rect::new(0, 0, 20, 6); // 4 item rows
    menu.select_last();
    let (_, hits) = render(&menu, short);
    assert_eq!(
        hits.hit(2, 4),
        MenuHit::Item(9),
        "last row shows the selection"
    );
    assert_eq!(hits.hit(2, 1), MenuHit::Item(6));
}

// ---- submenus ----

fn submenu(label: &str, items: Vec<MenuItem>) -> MenuItem {
    MenuItem::Submenu {
        label: label.into(),
        items,
    }
}

/// `Fetch`, `Branch ▸ [Create, Delete]`, `---`, `Show Git Output`.
fn nested() -> PopupMenu {
    PopupMenu::new(
        vec![
            entry("Fetch"),
            submenu("Branch", vec![entry("Create"), entry("Delete")]),
            MenuItem::Separator,
            entry("Show Git Output"),
        ],
        Position::new(2, 1),
    )
}

#[test]
fn submenu_rows_are_selectable_and_open_with_right_or_enter() {
    let mut menu = nested();
    menu.handle_key(Key::Down);
    assert_eq!(menu.selected(), Some(1));
    assert!(!menu.is_child_open());
    assert_eq!(menu.handle_key(Key::Right), MenuOutcome::Open);
    assert!(menu.is_child_open());
    assert_eq!(menu.handle_key(Key::Left), MenuOutcome::Open);
    assert!(!menu.is_child_open(), "Left closes the child");
    assert_eq!(menu.handle_key(Key::Enter), MenuOutcome::Open);
    assert!(menu.is_child_open(), "Enter opens it too");
    assert_eq!(menu.handle_key(Key::Esc), MenuOutcome::Open);
    assert!(!menu.is_child_open(), "Esc closes only the child");
    assert_eq!(menu.handle_key(Key::Esc), MenuOutcome::Closed);
}

#[test]
fn keys_go_to_the_open_child_and_its_choice_comes_back() {
    let mut menu = nested();
    menu.handle_key(Key::Down);
    menu.handle_key(Key::Right);
    menu.handle_key(Key::Down);
    assert_eq!(chosen_label(menu.handle_key(Key::Enter)), "Delete");
    assert_eq!(menu.selected_label(), Some("Delete"));
}

#[test]
fn right_on_a_plain_entry_does_nothing() {
    let mut menu = nested();
    assert_eq!(menu.handle_key(Key::Right), MenuOutcome::Open);
    assert!(!menu.is_child_open());
    assert_eq!(menu.selected_label(), Some("Fetch"));
}

#[test]
fn submenu_rows_show_an_arrow_and_fit_it() {
    // "Show Git Output" (15) is the widest; "Branch ▸" needs 8.
    assert_eq!(nested().area(screen()).width, 19);
    let (buf, _) = render(&nested(), screen());
    let row: String = (2..21).map(|x| buf[(x, 3)].symbol()).collect::<String>();
    assert_eq!(row, "│ Branch        ▸ │");
}

#[test]
fn the_child_opens_beside_the_parent_row_or_flips_left() {
    let mut menu = nested();
    menu.handle_key(Key::Down);
    menu.handle_key(Key::Right);
    let (_, hits) = render(&menu, screen());
    let parent = hits.area();
    let child = hits.child().unwrap().area();
    assert_eq!(
        (child.x, child.y),
        (parent.right(), 2),
        "beside the Branch row"
    );

    let mut near_edge = PopupMenu::new(nested().items().to_vec(), Position::new(38, 1));
    near_edge.handle_key(Key::Down);
    near_edge.handle_key(Key::Right);
    let (_, hits) = render(&near_edge, screen());
    let parent = hits.area();
    let child = hits.child().unwrap().area();
    assert_eq!(child.right(), parent.x, "flipped to the left side");
}

#[test]
fn mouse_hover_opens_submenus_and_clicks_reach_the_child() {
    let mut menu = nested();
    let (_, hits) = render(&menu, screen());
    menu.handle_hover(&hits, 4, 3);
    assert!(menu.is_child_open(), "hovering Branch opens it");
    let (_, hits) = render(&menu, screen());
    let child = hits.child().unwrap().area();
    assert_eq!(
        chosen_label(menu.handle_click(&hits, child.x + 2, child.y + 2)),
        "Delete"
    );

    let (_, hits) = render(&menu, screen());
    menu.handle_hover(&hits, 4, 2);
    assert!(!menu.is_child_open(), "hovering another entry closes it");
    assert_eq!(menu.handle_click(&hits, 30, 10), MenuOutcome::Closed);
}

#[test]
fn clicking_a_submenu_row_opens_it() {
    let mut menu = nested();
    let (_, hits) = render(&menu, screen());
    assert_eq!(menu.handle_click(&hits, 4, 3), MenuOutcome::Open);
    assert!(menu.is_child_open());
}
