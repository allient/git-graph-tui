//! Renders real repositories into an in-memory terminal buffer.

// Test helpers panic on bad fixtures, like the tests themselves.
#![expect(clippy::unwrap_used, reason = "test helpers")]

use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tgg_git::{log, refs, status};
use tgg_graph::{Graph, Row};
use tgg_model::{Commit, Oid, Ref, WorkingTreeStatus};
use tgg_testkit::FixtureRepo;
use tgg_ui::{Gap, GraphView, HitTarget, Theme};

struct Loaded {
    commits: Vec<Commit>,
    rows: Vec<Row>,
    refs: Vec<Ref>,
    status: WorkingTreeStatus,
    head: Option<Oid>,
}

fn load(repo: &FixtureRepo) -> Loaded {
    let run = |args: Vec<String>| {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        repo.git_raw(&args)
    };
    let commits = log::parse(&run(log::args(&log::LogQuery {
        max_count: 100,
        skip: 0,
    })))
    .unwrap();
    let refs = refs::parse(&run(refs::args())).unwrap();
    let status = status::parse(&run(status::args())).unwrap();
    let head = Oid::parse(repo.git(&["rev-parse", "HEAD"]).as_str()).ok();
    let mut graph = Graph::new(head.clone());
    graph.extend(&commits);
    Loaded {
        rows: graph.rows().to_vec(),
        commits,
        refs,
        status,
        head,
    }
}

fn view(l: &Loaded) -> GraphView<'_> {
    GraphView {
        commits: &l.commits,
        rows: &l.rows,
        refs: &l.refs,
        head: l.head.as_ref(),
        uncommitted: l.status.changed_files(),
        offset: 0,
        selected: 0,
        gap: None,
    }
}

fn text(buf: &Buffer) -> String {
    let area = buf.area;
    (area.top()..area.bottom())
        .map(|y| {
            (area.left()..area.right())
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render(v: &GraphView<'_>, width: u16, height: u16) -> (Buffer, tgg_ui::HitMap) {
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    let hits = v.render(area, &mut buf, &Theme::default());
    (buf, hits)
}

#[test]
fn sample_repo_wide() {
    let repo = FixtureRepo::sample();
    let loaded = load(&repo);
    let (buf, _) = render(&view(&loaded), 110, 10);
    insta::assert_snapshot!(text(&buf));
}

#[test]
fn narrow_terminals_drop_author_then_date() {
    let repo = FixtureRepo::sample();
    let loaded = load(&repo);
    let (buf, _) = render(&view(&loaded), 60, 10);
    insta::assert_snapshot!(text(&buf));
}

#[test]
fn uncommitted_row_comes_first_when_the_tree_is_dirty() {
    let repo = FixtureRepo::sample();
    let loaded = load(&repo);
    let (buf, hits) = render(&view(&loaded), 110, 10);
    assert!(
        text(&buf)
            .lines()
            .next()
            .unwrap()
            .contains("Uncommitted Changes (1)")
    );
    assert_eq!(hits.hit(40, 0), Some(&HitTarget::Uncommitted));
}

#[test]
fn clean_tree_has_no_uncommitted_row() {
    let repo = FixtureRepo::new();
    repo.commit_file("a.txt", "1\n", "Only commit");
    let loaded = load(&repo);
    let (buf, hits) = render(&view(&loaded), 80, 3);
    assert!(text(&buf).lines().next().unwrap().contains("Only commit"));
    assert_eq!(hits.hit(40, 0), Some(&HitTarget::Commit(0)));
}

#[test]
fn hit_map_finds_commits_and_badges() {
    let repo = FixtureRepo::sample();
    let loaded = load(&repo);
    let (buf, hits) = render(&view(&loaded), 110, 10);
    let screen = text(&buf);

    // Row 1 is the first commit ("Add login" on feature/login).
    assert_eq!(hits.hit(100, 1), Some(&HitTarget::Commit(0)));

    // Clicking inside a badge targets that ref.
    let (y, line) = screen
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains(" main "))
        .unwrap();
    let x = line.find(" main ").unwrap() + 2;
    let x = u16::try_from(line[..x].chars().count()).unwrap();
    let y = u16::try_from(y).unwrap();
    match hits.hit(x, y) {
        Some(HitTarget::Ref { full_name, .. }) => assert_eq!(full_name, "refs/heads/main"),
        other => panic!("expected the main badge, got {other:?}"),
    }
    assert_eq!(hits.hit(200, 200), None);
}

#[test]
fn scrolling_starts_at_the_offset() {
    let repo = FixtureRepo::sample();
    let loaded = load(&repo);
    let mut v = view(&loaded);
    v.offset = 3;
    let (buf, hits) = render(&v, 110, 4);
    // Display row 3 is commit index 2 (row 0 is the uncommitted row).
    assert!(
        text(&buf)
            .lines()
            .next()
            .unwrap()
            .contains("Merge branch 'develop'")
    );
    assert_eq!(hits.hit(100, 0), Some(&HitTarget::Commit(2)));
}

#[test]
fn selected_row_is_highlighted() {
    let repo = FixtureRepo::sample();
    let loaded = load(&repo);
    let mut v = view(&loaded);
    v.selected = 2;
    let (buf, _) = render(&v, 110, 10);
    let theme = Theme::default();
    assert_eq!(buf[(105, 2)].bg, theme.selected_bg);
    assert_ne!(buf[(105, 3)].bg, theme.selected_bg);
}

#[test]
fn commits_outside_head_ancestry_are_muted() {
    let repo = FixtureRepo::sample();
    let loaded = load(&repo);
    let (buf, _) = render(&view(&loaded), 110, 10);
    let screen = text(&buf);
    let theme = Theme::default();
    // "Add login" (feature/login) isn't reachable from HEAD (main).
    let y = u16::try_from(
        screen
            .lines()
            .position(|l| l.contains("Add login"))
            .unwrap(),
    )
    .unwrap();
    let line = screen.lines().nth(y.into()).unwrap();
    let x = u16::try_from(line[..line.find("Add login").unwrap()].chars().count()).unwrap();
    assert_eq!(buf[(x, y)].fg, theme.muted);
}

#[test]
fn a_gap_opens_blank_lines_below_its_row() {
    let repo = FixtureRepo::sample();
    let loaded = load(&repo);
    let plain = view(&loaded);
    let gapped = GraphView {
        gap: Some(Gap {
            after: 1,
            height: 3,
        }),
        ..view(&loaded)
    };
    let (plain_buf, _) = render(&plain, 100, 10);
    let (buf, hits) = render(&gapped, 100, 10);
    let plain_lines: Vec<String> = text(&plain_buf).lines().map(str::to_owned).collect();
    let lines: Vec<String> = text(&buf).lines().map(str::to_owned).collect();

    // Rows 0 and 1, three blank lines, then row 2 onwards.
    assert_eq!(lines[..2], plain_lines[..2]);
    assert!(lines[2..5].iter().all(String::is_empty), "{lines:#?}");
    assert_eq!(lines[5..], plain_lines[2..7]);
    assert_eq!(hits.gap(), Some(Rect::new(0, 2, 100, 3)));
    // Clicks below the gap resolve to the shifted row (display 2 is commit 1).
    assert_eq!(hits.hit(40, 5), Some(&HitTarget::Commit(1)));
    assert_eq!(hits.hit(40, 3), None);
}

#[test]
fn a_gap_is_clipped_at_the_bottom_and_skipped_when_its_row_is_off_screen() {
    let repo = FixtureRepo::sample();
    let loaded = load(&repo);
    let clipped = GraphView {
        gap: Some(Gap {
            after: 2,
            height: 10,
        }),
        ..view(&loaded)
    };
    let (_, hits) = render(&clipped, 100, 6);
    assert_eq!(hits.gap(), Some(Rect::new(0, 3, 100, 3)));

    let hidden = GraphView {
        offset: 3,
        gap: Some(Gap {
            after: 1,
            height: 3,
        }),
        ..view(&loaded)
    };
    let (_, hits) = render(&hidden, 100, 6);
    assert_eq!(hits.gap(), None);
}
