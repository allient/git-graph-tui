//! Renders the real S8/S9 context menus into an in-memory buffer.

#![expect(clippy::unwrap_used, reason = "test helpers")]

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use tgg_actions::{CommitTarget, MenuContext, Target, menu_for};
use tgg_model::{HeadState, Oid};
use tgg_ui::{PopupMenu, Theme};

fn oid(c: char) -> Oid {
    Oid::parse(&c.to_string().repeat(40)).unwrap()
}

fn ctx() -> MenuContext {
    MenuContext {
        head: HeadState::Branch("main".into()),
        head_commit: Some(oid('a')),
        remotes: vec!["origin".into()],
        branch_filter: Vec::new(),
    }
}

/// Renders `menu` over a screen filled with dots, so the cleared box shows.
fn render(menu: &PopupMenu, width: u16, height: u16) -> String {
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
    menu.render(area, &mut buf, &Theme::default());
    (0..height)
        .map(|y| (0..width).map(|x| buf[(x, y)].symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn branch_menu() {
    let target = Target::LocalBranch {
        name: "develop".into(),
        target: oid('b'),
    };
    let menu = PopupMenu::new(menu_for(&target, &ctx()), Position::new(2, 1));
    insta::assert_snapshot!(render(&menu, 44, 15));
}

#[test]
fn commit_menu_with_a_disabled_drop() {
    let target = Target::Commit(CommitTarget {
        id: oid('c'),
        subject: "Add lib".into(),
        parent_count: 2,
        is_head_ancestor: false,
    });
    let menu = PopupMenu::new(menu_for(&target, &ctx()), Position::new(1, 0));
    insta::assert_snapshot!(render(&menu, 48, 18));
}

#[test]
fn menu_near_the_bottom_right_corner_flips_up_and_left() {
    let target = Target::LocalBranch {
        name: "main".into(),
        target: oid('a'),
    };
    let menu = PopupMenu::new(menu_for(&target, &ctx()), Position::new(38, 11));
    insta::assert_snapshot!(render(&menu, 40, 12));
}
