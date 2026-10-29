use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tgg_git::show::{CommitDetails, FileChange, Numstat};
use tgg_model::{Change, Commit, Oid, Signature};

use super::*;
use crate::{Key, Theme};

fn oid(c: char) -> Oid {
    Oid::parse(&c.to_string().repeat(40)).unwrap()
}

fn file(path: &str, added: u32, deleted: u32) -> FileChange {
    FileChange {
        path: path.into(),
        old_path: None,
        status: Change::Modified,
        stat: Numstat::Lines { added, deleted },
    }
}

fn details(parents: Vec<Oid>) -> CommitDetails {
    let sig = |name: &str| Signature {
        name: name.into(),
        email: format!("{}@x.io", name.to_lowercase()),
        time: 1_767_225_660,
    };
    CommitDetails {
        commit: Commit {
            id: oid('a'),
            parents,
            author: sig("Ann"),
            committer: sig("Com"),
            subject: "Fix login".into(),
        },
        message: "Fix login\n\nThe session expired too early.".into(),
    }
}

/// `apps/full-app/{b.rs,a.rs}`, `apps/z.rs`, `README.md`, `docs/guide.md`.
fn files() -> Vec<FileChange> {
    vec![
        file("README.md", 1, 0),
        file("apps/full-app/b.rs", 2, 1),
        file("apps/z.rs", 0, 3),
        file("docs/guide.md", 5, 0),
        file("apps/full-app/a.rs", 1, 1),
    ]
}

fn view() -> CommitDetailsView {
    CommitDetailsView::new(details(vec![oid('b'), oid('c')]), files())
}

fn labels(view: &CommitDetailsView) -> Vec<String> {
    view.rows()
        .iter()
        .map(|row| match row {
            TreeRow::Dir { depth, name } => format!("{}{name}/", "  ".repeat(*depth)),
            TreeRow::File { depth, index } => {
                format!("{}{}", "  ".repeat(*depth), view.files()[*index].path)
            }
        })
        .collect()
}

fn selected_path(view: &CommitDetailsView) -> &str {
    &view.selected_file().unwrap().path
}

// ---- tree ----

#[test]
fn files_are_grouped_into_a_directory_tree_folders_first() {
    assert_eq!(
        labels(&view()),
        [
            "apps/",
            "  full-app/",
            "    apps/full-app/a.rs",
            "    apps/full-app/b.rs",
            "  apps/z.rs",
            "docs/",
            "  docs/guide.md",
            "README.md",
        ]
    );
}

#[test]
fn file_labels_show_counts_renames_and_binaries() {
    let renamed = FileChange {
        path: "src/new.rs".into(),
        old_path: Some("src/old.rs".into()),
        status: Change::Renamed,
        stat: Numstat::Lines {
            added: 1,
            deleted: 2,
        },
    };
    let moved = FileChange {
        path: "lib/new.rs".into(),
        old_path: Some("src/old.rs".into()),
        status: Change::Renamed,
        stat: Numstat::Binary,
    };
    assert_eq!(
        file_label(&renamed),
        ("old.rs → new.rs".to_owned(), "(+1 -2)".to_owned())
    );
    assert_eq!(
        file_label(&moved),
        ("new.rs ← src/old.rs".to_owned(), "(binary)".to_owned())
    );
    assert_eq!(
        file_label(&file("a/b.txt", 0, 0)),
        ("b.txt".to_owned(), "(+0 -0)".to_owned())
    );
}

// ---- selection and keys ----

#[test]
fn selection_starts_on_the_first_file_and_skips_folders() {
    let mut view = view();
    assert_eq!(selected_path(&view), "apps/full-app/a.rs");
    view.handle_key(Key::Down);
    assert_eq!(selected_path(&view), "apps/full-app/b.rs");
    view.handle_key(Key::Down);
    assert_eq!(selected_path(&view), "apps/z.rs");
    view.handle_key(Key::Down);
    assert_eq!(selected_path(&view), "docs/guide.md", "skips docs/");
    view.handle_key(Key::End);
    assert_eq!(selected_path(&view), "README.md");
    view.handle_key(Key::Down);
    assert_eq!(selected_path(&view), "README.md", "stops at the end");
    view.handle_key(Key::Home);
    assert_eq!(selected_path(&view), "apps/full-app/a.rs");
    view.handle_key(Key::Up);
    assert_eq!(
        selected_path(&view),
        "apps/full-app/a.rs",
        "stops at the start"
    );
}

#[test]
fn enter_opens_the_selected_file_and_escape_closes() {
    let mut view = view();
    view.handle_key(Key::Down);
    assert_eq!(
        view.handle_key(Key::Enter),
        DetailsOutcome::OpenFile(file("apps/full-app/b.rs", 2, 1))
    );
    assert_eq!(view.handle_key(Key::Esc), DetailsOutcome::Close);
    assert_eq!(view.handle_key(Key::Char('x')), DetailsOutcome::Stay);
}

#[test]
fn a_commit_without_files_has_no_selection() {
    let mut view = CommitDetailsView::new(details(vec![]), Vec::new());
    assert!(view.selected_file().is_none());
    assert_eq!(view.handle_key(Key::Enter), DetailsOutcome::Stay);
}

// ---- rendering and mouse ----

fn render(view: &CommitDetailsView, area: Rect) -> (Buffer, DetailsHitMap) {
    let mut buf = Buffer::empty(area);
    let hits = view.render(area, &mut buf, &Theme::default());
    (buf, hits)
}

#[test]
fn clicking_a_parent_jumps_to_it_and_clicking_a_file_opens_it() {
    let mut view = view();
    let (_, hits) = render(&view, Rect::new(0, 5, 100, 12));
    let parent = hits.parent(&oid('c')).unwrap();
    assert_eq!(
        view.handle_click(&hits, parent.x, parent.y),
        DetailsOutcome::JumpTo(oid('c'))
    );
    let readme = hits.file_row(7).unwrap();
    assert_eq!(
        view.handle_click(&hits, readme.x, readme.y),
        DetailsOutcome::OpenFile(file("README.md", 1, 0))
    );
    assert_eq!(selected_path(&view), "README.md");
    let folder = hits.file_row(0);
    assert!(folder.is_none(), "folders are not clickable");
    assert_eq!(view.handle_click(&hits, 0, 0), DetailsOutcome::Stay);
    assert_eq!(hits.hit(0, 0), DetailsHit::Outside);
}

#[test]
fn the_selected_file_is_highlighted() {
    let view = view();
    let (buf, hits) = render(&view, Rect::new(0, 0, 100, 12));
    let row = hits.file_row(2).unwrap();
    assert_eq!(buf[(row.x + 1, row.y)].bg, Theme::default().selected_bg);
}

#[test]
fn long_file_lists_scroll_to_the_selection() {
    let many: Vec<FileChange> = (0..30)
        .map(|i| file(&format!("f{i:02}.txt"), 1, 0))
        .collect();
    let mut view = CommitDetailsView::new(details(vec![oid('b')]), many);
    view.handle_key(Key::End);
    let (_, hits) = render(&view, Rect::new(0, 0, 100, 8));
    assert!(hits.file_row(29).is_some(), "last file is visible");
    assert!(hits.file_row(0).is_none());
}

fn line(buf: &Buffer, y: u16) -> String {
    (buf.area.left()..buf.area.right())
        .map(|x| buf[(x, y)].symbol())
        .collect::<String>()
        .trim_end()
        .to_owned()
}

#[test]
fn v_views_the_selected_file() {
    let mut view = view();
    assert_eq!(
        view.handle_key(Key::Char('v')),
        DetailsOutcome::ViewFile(file("apps/full-app/a.rs", 1, 1))
    );
}

#[test]
fn a_comparison_lists_its_files_across_the_whole_panel() {
    let mut view = CommitDetailsView::comparison("Comparing aaaaaaa..bbbbbbb".into(), files());
    assert!(view.details().is_none());
    let (buf, hits) = render(&view, Rect::new(0, 0, 90, 6));
    let title = line(&buf, 0);
    assert!(
        title.starts_with(" Comparing aaaaaaa..bbbbbbb (5 changed files) "),
        "{title:?}"
    );
    // The hints end where the side buttons start.
    assert!(
        title.ends_with(" enter diff  t difftool  esc close  ✕"),
        "{title:?}"
    );
    assert!(
        line(&buf, 1).starts_with(" ▾ apps/ "),
        "{:?}",
        line(&buf, 1)
    );
    assert!(
        line(&buf, 3).starts_with("     · a.rs"),
        "{:?}",
        line(&buf, 3)
    );
    // Enter opens the file; there is no single commit to view it at.
    assert_eq!(view.handle_key(Key::Char('v')), DetailsOutcome::Stay);
    let row = hits.file_row(2).unwrap();
    assert_eq!(
        view.handle_click(&hits, row.x, row.y),
        DetailsOutcome::OpenFile(file("apps/full-app/a.rs", 1, 1))
    );

    let one = CommitDetailsView::comparison("Comparing a..b".into(), vec![file("x", 1, 0)]);
    let (buf, _) = render(&one, Rect::new(0, 0, 80, 3));
    assert!(line(&buf, 0).starts_with(" Comparing a..b (1 changed file) "));
}

// ---- issue links (S20) ----

fn with_message(message: &str) -> CommitDetailsView {
    let mut info = details(vec![oid('b')]);
    info.message = message.into();
    CommitDetailsView::new(info, files())
}

/// Where `text` starts in the rendered buffer.
fn position(buf: &Buffer, text: &str) -> (u16, u16) {
    (buf.area.top()..buf.area.bottom())
        .find_map(|y| {
            let row = line(buf, y);
            let byte = row.find(text)?;
            Some((u16::try_from(row[..byte].chars().count()).unwrap(), y))
        })
        .unwrap_or_else(|| panic!("{text:?} not drawn"))
}

#[test]
fn issue_references_in_the_message_are_underlined_links() {
    let mut view = with_message("Fix login (#12)\n\nabc#34 is not one, but #5 is.");
    let (buf, hits) = render(&view, Rect::new(0, 0, 100, 12));
    let (x, y) = position(&buf, "#12");
    for dx in 0..3 {
        let cell = &buf[(x + dx, y)];
        assert!(cell.modifier.contains(Modifier::UNDERLINED), "x={}", x + dx);
        assert_eq!(cell.fg, Theme::default().tag);
    }
    assert!(!buf[(x + 3, y)].modifier.contains(Modifier::UNDERLINED));
    assert_eq!(
        view.handle_click(&hits, x + 1, y),
        DetailsOutcome::OpenIssue("12".into())
    );
    let (x, y) = position(&buf, "#5");
    assert_eq!(
        view.handle_click(&hits, x, y),
        DetailsOutcome::OpenIssue("5".into())
    );
    let (x, y) = position(&buf, "#34");
    assert!(!buf[(x, y)].modifier.contains(Modifier::UNDERLINED));
    assert_eq!(view.handle_click(&hits, x + 1, y), DetailsOutcome::Stay);
}

#[test]
fn a_configured_prefix_links_its_own_references() {
    let mut view = with_message("PROJ-7: Fix login #12").with_issue_prefix("PROJ-");
    let (buf, hits) = render(&view, Rect::new(0, 0, 100, 12));
    let (x, y) = position(&buf, "PROJ-7");
    assert_eq!(
        view.handle_click(&hits, x + 5, y),
        DetailsOutcome::OpenIssue("7".into())
    );
    let (x, y) = position(&buf, "#12");
    assert_eq!(view.handle_click(&hits, x, y), DetailsOutcome::Stay);
}

#[test]
fn a_link_cut_off_by_the_panel_edge_is_not_clickable() {
    let mut view = with_message("A long subject that ends with #12345");
    // The metadata half is 18 columns: the reference doesn't fit.
    let (buf, hits) = render(&view, Rect::new(0, 0, 40, 12));
    let y = position(&buf, "A long").1;
    for x in 0..18 {
        assert_eq!(
            view.handle_click(&hits, x, y),
            DetailsOutcome::Stay,
            "x={x}"
        );
    }
}

// ---- Git Graph look ----

fn render_with(view: &CommitDetailsView, area: Rect, theme: &Theme) -> Buffer {
    let mut buf = Buffer::empty(area);
    view.render(area, &mut buf, theme);
    buf
}

/// The right half of a 100-column panel (the file tree), from `x = 49`.
fn tree(buf: &Buffer, y: u16) -> String {
    line(buf, y).chars().skip(49).collect()
}

#[test]
fn field_labels_are_bold_and_values_are_plain() {
    let (buf, _) = render(&view(), Rect::new(0, 0, 100, 12));
    for label in ["Commit:", "Parents:", "Author:", "Committer:", "Date:"] {
        let (x, y) = position(&buf, label);
        for dx in 0..u16::try_from(label.len()).unwrap() {
            let cell = &buf[(x + dx, y)];
            assert!(cell.modifier.contains(Modifier::BOLD), "{label} x={dx}");
        }
    }
    let (x, y) = position(&buf, "Ann <");
    assert!(!buf[(x, y)].modifier.contains(Modifier::BOLD));
    assert_eq!(buf[(x, y)].fg, Theme::default().text);
}

#[test]
fn parent_hashes_and_emails_are_underlined_links() {
    let theme = Theme::default();
    let (buf, hits) = render(&view(), Rect::new(0, 0, 100, 12));
    let parent = hits.parent(&oid('c')).unwrap();
    for dx in 0..parent.width {
        let cell = &buf[(parent.x + dx, parent.y)];
        assert_eq!(cell.fg, theme.link, "x={dx}");
        assert!(cell.modifier.contains(Modifier::UNDERLINED));
    }
    for email in ["ann@x.io", "com@x.io"] {
        let (x, y) = position(&buf, email);
        for dx in 0..u16::try_from(email.len()).unwrap() {
            let cell = &buf[(x + dx, y)];
            assert_eq!(cell.fg, theme.link, "{email} x={dx}");
            assert!(cell.modifier.contains(Modifier::UNDERLINED), "{email}");
        }
        let around = [&buf[(x - 1, y)], &buf[(x + 8, y)]];
        assert!(
            around
                .iter()
                .all(|c| !c.modifier.contains(Modifier::UNDERLINED)),
            "only the address: {email}"
        );
    }
    let (x, y) = position(&buf, "Ann <");
    assert!(!buf[(x, y)].modifier.contains(Modifier::UNDERLINED));
}

#[test]
fn folders_and_files_have_their_icons() {
    let (buf, _) = render(&view(), Rect::new(0, 0, 100, 12));
    assert!(tree(&buf, 1).starts_with(" ▾ apps/"), "{:?}", tree(&buf, 1));
    assert!(
        tree(&buf, 2).starts_with("   ▾ full-app/"),
        "{:?}",
        tree(&buf, 2)
    );
    assert!(
        tree(&buf, 3).starts_with("     · a.rs"),
        "{:?}",
        tree(&buf, 3)
    );

    let nerd = Theme {
        icons: crate::Icons::Nerd,
        ..Theme::default()
    };
    let buf = render_with(&view(), Rect::new(0, 0, 100, 12), &nerd);
    assert!(
        tree(&buf, 1).starts_with(" \u{ea83} apps/"),
        "{:?}",
        tree(&buf, 1)
    );
    assert!(
        tree(&buf, 3).starts_with("     \u{ea7b} a.rs"),
        "{:?}",
        tree(&buf, 3)
    );
}

#[test]
fn file_names_are_colored_by_what_the_commit_did() {
    let theme = Theme::default();
    let with = |path: &str, status| FileChange {
        status,
        ..file(path, 1, 0)
    };
    let files = vec![
        with("added.rs", Change::Added),
        with("deleted.rs", Change::Deleted),
        with("modified.rs", Change::Modified),
        FileChange {
            old_path: Some("old.rs".into()),
            ..with("renamed.rs", Change::Renamed)
        },
    ];
    let view = CommitDetailsView::comparison("Comparing a..b".into(), files);
    let (buf, _) = render(&view, Rect::new(0, 0, 80, 6));
    for (name, color) in [
        ("added.rs", theme.added),
        ("deleted.rs", theme.deleted),
        ("modified.rs", theme.modified),
        ("old.rs → renamed.rs", theme.renamed),
    ] {
        let (x, y) = position(&buf, name);
        assert_eq!(buf[(x, y)].fg, color, "{name}");
        assert_eq!(buf[(x + 3, y)].fg, color, "{name}");
    }
}

// ---- counts in color (S25) ----

#[test]
fn counts_follow_the_name_split_by_a_pipe_and_new_files_have_none() {
    let files = vec![
        file("b.txt", 3, 2),
        FileChange {
            status: Change::Added,
            ..file("new.txt", 4, 0)
        },
        FileChange {
            stat: Numstat::Binary,
            ..file("logo.png", 0, 0)
        },
    ];
    let view = CommitDetailsView::comparison("Comparing a..b".into(), files);
    let (buf, _) = render(&view, Rect::new(0, 0, 80, 6));
    // Left of the side buttons.
    let files = |y: u16| -> String {
        let text: String = line(&buf, y).chars().take(77).collect();
        text.trim_end().to_owned()
    };
    assert_eq!(files(1), " · b.txt (+3 | -2)");
    assert_eq!(files(2), " · logo.png (binary)");
    assert_eq!(files(3), " · new.txt");
    // The diff header keeps its own format.
    assert_eq!(file_label(&file("b.txt", 3, 2)).1, "(+3 -2)");
}

#[test]
fn added_lines_are_green_and_deleted_lines_red() {
    let theme = Theme::default();
    let files = vec![
        file("changed.rs", 12, 3),
        FileChange {
            status: Change::Added,
            ..file("fresh.rs", 9, 0)
        },
    ];
    let view = CommitDetailsView::comparison("Comparing a..b".into(), files);
    let (buf, _) = render(&view, Rect::new(0, 0, 80, 6));
    let (x, y) = position(&buf, "(+12 | -3)");
    let fg = |dx: u16| buf[(x + dx, y)].fg;
    assert_eq!(fg(0), theme.text, "(");
    assert_eq!(
        (fg(1), fg(2), fg(3)),
        (theme.added, theme.added, theme.added)
    );
    assert_eq!((fg(4), fg(5), fg(6)), (theme.text, theme.text, theme.text));
    assert_eq!((fg(7), fg(8)), (theme.deleted, theme.deleted));
    assert_eq!(fg(9), theme.text, ")");
    let (x, y) = position(&buf, "fresh.rs");
    assert_eq!(buf[(x, y)].fg, theme.added);
    assert!(!line(&buf, y).contains("(+"), "{:?}", line(&buf, y));
}

// ---- side buttons (S26) ----

/// The button column of a panel drawn in `area`: x of the icons.
fn buttons_x(area: Rect) -> u16 {
    area.right() - 2
}

#[test]
fn close_find_tree_and_list_buttons_sit_at_the_right_edge() {
    let area = Rect::new(10, 3, 100, 12);
    let (buf, hits) = render(&view(), area);
    let x = buttons_x(area);
    let icons: Vec<&str> = (0..4).map(|dy| buf[(x, area.y + dy)].symbol()).collect();
    assert_eq!(icons, ["✕", "⌕", "├", "≡"]);
    for dy in 0..4 {
        assert_eq!(buf[(x + 1, area.y + dy)].symbol(), " ", "blank after icon");
    }
    let theme = Theme::default();
    assert_eq!(buf[(x, area.y + 2)].bg, theme.selected_bg, "tree is active");
    assert_ne!(buf[(x, area.y + 3)].bg, theme.selected_bg);
    assert_eq!(
        hits.hit(x, area.y),
        DetailsHit::Button(DetailsButton::Close)
    );
    assert_eq!(
        hits.hit(x + 1, area.y + 3),
        DetailsHit::Button(DetailsButton::List)
    );

    let nerd = Theme {
        icons: crate::Icons::Nerd,
        ..Theme::default()
    };
    let buf = render_with(&view(), area, &nerd);
    let icons: Vec<&str> = (0..4).map(|dy| buf[(x, area.y + dy)].symbol()).collect();
    assert_eq!(icons, ["\u{ea76}", "\u{ea6d}", "\u{eb86}", "\u{eb84}"]);
}

#[test]
fn the_close_button_closes_and_the_list_button_shows_full_paths() {
    let area = Rect::new(0, 0, 100, 12);
    let mut view = view();
    let (_, hits) = render(&view, area);
    let x = buttons_x(area);
    assert_eq!(view.handle_click(&hits, x, 0), DetailsOutcome::Close);
    assert_eq!(view.handle_click(&hits, x, 3), DetailsOutcome::Stay);
    assert_eq!(view.layout(), FileLayout::List);
    assert_eq!(
        labels(&view),
        [
            "README.md",
            "apps/full-app/a.rs",
            "apps/full-app/b.rs",
            "apps/z.rs",
            "docs/guide.md",
        ]
    );
    let (buf, hits) = render(&view, area);
    assert!(
        tree(&buf, 1).starts_with(" · README.md"),
        "{:?}",
        tree(&buf, 1)
    );
    assert!(
        tree(&buf, 2).starts_with(" · apps/full-app/a.rs"),
        "{:?}",
        tree(&buf, 2)
    );
    assert_eq!(
        buf[(x, 3)].bg,
        Theme::default().selected_bg,
        "list is active"
    );
    assert_eq!(view.handle_click(&hits, x, 2), DetailsOutcome::Stay);
    assert_eq!(view.layout(), FileLayout::Tree);
    assert_eq!(labels(&view)[0], "apps/");
}

#[test]
fn l_switches_between_the_tree_and_the_list_keeping_the_selection() {
    let mut view = view();
    view.handle_key(Key::Down);
    assert_eq!(selected_path(&view), "apps/full-app/b.rs");
    view.handle_key(Key::Char('l'));
    assert_eq!(view.layout(), FileLayout::List);
    assert_eq!(selected_path(&view), "apps/full-app/b.rs");
    view.handle_key(Key::Char('l'));
    assert_eq!(view.layout(), FileLayout::Tree);
    assert_eq!(selected_path(&view), "apps/full-app/b.rs");
}

#[test]
fn find_filters_the_files_by_name_and_escape_clears_it() {
    let area = Rect::new(0, 0, 100, 12);
    let mut view = view();
    let (_, hits) = render(&view, area);
    assert_eq!(
        view.handle_click(&hits, buttons_x(area), 1),
        DetailsOutcome::Stay
    );
    assert!(view.is_filtering());
    for c in "GUI".chars() {
        assert_eq!(view.handle_key(Key::Char(c)), DetailsOutcome::Stay);
    }
    assert_eq!(view.filter(), "GUI");
    assert_eq!(labels(&view), ["docs/", "  docs/guide.md"], "ignores case");
    assert_eq!(selected_path(&view), "docs/guide.md");
    let (buf, _) = render(&view, area);
    assert!(
        tree(&buf, 0).starts_with(" Changed files (1 of 5)  find: GUI"),
        "{:?}",
        tree(&buf, 0)
    );
    view.handle_key(Key::Backspace);
    view.handle_key(Key::Backspace);
    assert_eq!(view.filter(), "G");
    // `l` is typed into the filter while it is edited.
    view.handle_key(Key::Char('l'));
    assert_eq!(view.filter(), "Gl");
    assert_eq!(view.layout(), FileLayout::Tree);
    // Enter keeps the filter and stops editing it.
    assert_eq!(view.handle_key(Key::Enter), DetailsOutcome::Stay);
    assert!(!view.is_filtering());
    assert_eq!(view.filter(), "Gl");
    // `f` edits it again; Esc clears it, a second Esc closes the panel.
    view.handle_key(Key::Char('f'));
    assert!(view.is_filtering());
    assert_eq!(view.handle_key(Key::Esc), DetailsOutcome::Stay);
    assert_eq!(view.filter(), "");
    assert_eq!(labels(&view).len(), 8);
    assert_eq!(view.handle_key(Key::Esc), DetailsOutcome::Close);
}

#[test]
fn a_filter_matching_nothing_leaves_no_selection() {
    let mut view = view();
    view.handle_key(Key::Char('f'));
    view.handle_key(Key::Char('q'));
    assert!(view.rows().is_empty());
    assert!(view.selected_file().is_none());
    assert_eq!(view.handle_key(Key::Enter), DetailsOutcome::Stay);
    assert_eq!(view.handle_key(Key::Enter), DetailsOutcome::Stay, "no file");
    view.handle_key(Key::Char('f'));
    view.handle_key(Key::Backspace);
    assert_eq!(selected_path(&view), "apps/full-app/a.rs");
}

// ---- internal scroll (S24) ----

fn long_message() -> CommitDetailsView {
    let body = (1..=30)
        .map(|i| format!("Line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let many: Vec<FileChange> = (0..30)
        .map(|i| file(&format!("f{i:02}.txt"), 1, 0))
        .collect();
    let mut info = details(vec![oid('b')]);
    info.message = format!("Subject\n\n{body}");
    CommitDetailsView::new(info, many)
}

#[test]
fn the_message_and_the_files_scroll_on_their_own() {
    let area = Rect::new(0, 0, 100, 8);
    let mut view = long_message();
    let (buf, hits) = render(&view, area);
    assert!(line(&buf, 0).starts_with(" Commit:"), "{:?}", line(&buf, 0));
    // Wheel over the message: only the left side moves.
    assert!(view.scroll(&hits, 5, 3, 4));
    let (buf, hits) = render(&view, area);
    assert!(line(&buf, 0).starts_with(" Date:"), "{:?}", line(&buf, 0));
    assert!(
        tree(&buf, 1).starts_with(" · f00.txt"),
        "{:?}",
        tree(&buf, 1)
    );
    // Wheel over the files: only the file list moves.
    assert!(view.scroll(&hits, 60, 3, 3));
    let (buf, hits) = render(&view, area);
    assert!(line(&buf, 0).starts_with(" Date:"), "{:?}", line(&buf, 0));
    assert!(
        tree(&buf, 1).starts_with(" · f03.txt"),
        "{:?}",
        tree(&buf, 1)
    );
    assert!(hits.file_row(0).is_none());
    // Both stop at their ends.
    assert!(view.scroll(&hits, 5, 3, 1000));
    let (buf, hits) = render(&view, area);
    assert!(line(&buf, 7).starts_with(" Line 30"), "{:?}", line(&buf, 7));
    assert!(view.scroll(&hits, 60, 3, 1000));
    let (_, hits) = render(&view, area);
    assert!(hits.file_row(29).is_some());
    assert!(view.scroll(&hits, 60, 3, -1000));
    let (_, hits) = render(&view, area);
    assert!(hits.file_row(0).is_some());
    // Outside the panel nothing scrolls.
    assert!(!view.scroll(&hits, 5, 20, 3));
}

#[test]
fn moving_the_selection_brings_it_back_into_view() {
    let area = Rect::new(0, 0, 100, 8);
    let mut view = long_message();
    let (_, hits) = render(&view, area);
    view.scroll(&hits, 60, 3, 20);
    let (_, hits) = render(&view, area);
    assert!(hits.file_row(0).is_none());
    view.handle_key(Key::Home);
    let (_, hits) = render(&view, area);
    assert!(hits.file_row(0).is_some());
}

#[test]
fn the_message_scrolls_by_lines_from_the_keyboard() {
    let area = Rect::new(0, 0, 100, 8);
    let mut view = long_message();
    let (_, hits) = render(&view, area);
    view.scroll_message(&hits, 2);
    let (buf, hits) = render(&view, area);
    assert!(line(&buf, 0).starts_with(" Author:"), "{:?}", line(&buf, 0));
    view.scroll_message(&hits, -5);
    let (buf, _) = render(&view, area);
    assert!(line(&buf, 0).starts_with(" Commit:"), "{:?}", line(&buf, 0));
}
