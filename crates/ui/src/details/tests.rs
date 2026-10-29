use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tgg_git::show::{CommitDetails, FileChange, Numstat};
use tgg_model::{Commit, Oid, Signature};

use super::*;
use crate::{Key, Theme};

fn oid(c: char) -> Oid {
    Oid::parse(&c.to_string().repeat(40)).unwrap()
}

fn file(path: &str, added: u32, deleted: u32) -> FileChange {
    FileChange {
        path: path.into(),
        old_path: None,
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
        stat: Numstat::Lines {
            added: 1,
            deleted: 2,
        },
    };
    let moved = FileChange {
        path: "lib/new.rs".into(),
        old_path: Some("src/old.rs".into()),
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
    let (buf, hits) = render(&view, Rect::new(0, 0, 80, 6));
    let title = line(&buf, 0);
    assert!(
        title.starts_with(" Comparing aaaaaaa..bbbbbbb (5 changed files) "),
        "{title:?}"
    );
    assert!(
        title.ends_with(" enter diff  t difftool  esc close"),
        "{title:?}"
    );
    assert_eq!(line(&buf, 1), " apps/");
    assert!(
        line(&buf, 3).starts_with("     a.rs"),
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
