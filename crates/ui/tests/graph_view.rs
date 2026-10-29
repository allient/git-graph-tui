//! Renders real repositories into an in-memory terminal buffer.

// Test helpers panic on bad fixtures, like the tests themselves.
#![expect(clippy::unwrap_used, reason = "test helpers")]

use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use tgg_actions::GraphColumn;
use tgg_git::{log, refs, status};
use tgg_graph::{Graph, Row};
use tgg_model::{Commit, Oid, Ref, WorkingTreeStatus};
use tgg_testkit::FixtureRepo;
use tgg_ui::{ColumnDivider, ColumnWidths, Gap, GraphView, HitTarget, Icons, Theme};

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
        revisions: log::Revisions::default(),
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
        widths: ColumnWidths::default(),
        matches: &[],
        hidden: &[],
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
    render_with(v, width, height, &Theme::default())
}

fn render_with(
    v: &GraphView<'_>,
    width: u16,
    height: u16,
    theme: &Theme,
) -> (Buffer, tgg_ui::HitMap) {
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    let hits = v.render(area, &mut buf, theme);
    (buf, hits)
}

/// Where `needle` starts on the screen, in cells.
fn find(buf: &Buffer, needle: &str) -> (u16, u16) {
    let screen = text(buf);
    let (y, line) = screen
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains(needle))
        .unwrap_or_else(|| panic!("{needle:?} not on screen:\n{screen}"));
    let x = line[..line.find(needle).unwrap()].chars().count();
    (u16::try_from(x).unwrap(), u16::try_from(y).unwrap())
}

fn modifiers(buf: &Buffer, x: u16, y: u16, len: u16) -> Vec<Modifier> {
    (x..x + len).map(|x| buf[(x, y)].modifier).collect()
}

#[test]
fn sample_repo_wide() {
    let repo = FixtureRepo::sample();
    let loaded = load(&repo);
    let (buf, _) = render(&view(&loaded), 110, 16);
    insta::assert_snapshot!(text(&buf));
}

#[test]
fn the_first_line_names_the_columns() {
    let repo = FixtureRepo::sample();
    let loaded = load(&repo);
    let (buf, _) = render(&view(&loaded), 110, 10);
    let header = text(&buf).lines().next().unwrap().to_owned();
    let names: Vec<&str> = header
        .split(|c: char| c.is_whitespace() || c == '│')
        .filter(|w| !w.is_empty())
        .collect();
    assert_eq!(names, ["Graph", "Description", "Date", "Author", "Commit"]);
    assert_eq!(buf[(0, 0)].bg, Theme::default().header_bg);
}

#[test]
fn a_line_joins_consecutive_commits() {
    let repo = FixtureRepo::sample();
    let loaded = load(&repo);
    let (buf, hits) = render(&view(&loaded), 110, 10);
    let lines: Vec<String> = text(&buf).lines().map(str::to_owned).collect();
    // "Add login" (line 3) and its parent "Return a value" (line 5), both on
    // lane 0, two columns in from the left edge.
    assert!(
        lines[3].starts_with("  ●") && lines[5].starts_with("  ●"),
        "{lines:#?}"
    );
    assert_eq!(lines[4], "  ┃");
    // The connector line belongs to the row above it.
    assert_eq!(hits.hit(100, 4), Some(&HitTarget::Commit(0)));
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
            .nth(1)
            .unwrap()
            .contains("Uncommitted Changes (1)")
    );
    assert_eq!(hits.hit(40, 1), Some(&HitTarget::Uncommitted));
}

#[test]
fn clean_tree_has_no_uncommitted_row() {
    let repo = FixtureRepo::new();
    repo.commit_file("a.txt", "1\n", "Only commit");
    let loaded = load(&repo);
    let (buf, hits) = render(&view(&loaded), 80, 3);
    assert!(text(&buf).lines().nth(1).unwrap().contains("Only commit"));
    assert_eq!(hits.hit(40, 1), Some(&HitTarget::Commit(0)));
}

#[test]
fn hit_map_finds_commits_and_badges() {
    let repo = FixtureRepo::sample();
    let loaded = load(&repo);
    let (buf, hits) = render(&view(&loaded), 110, 10);
    let screen = text(&buf);

    // Line 3 is the first commit ("Add login" on feature/login), below the
    // header, the uncommitted row and its connector.
    assert_eq!(hits.hit(100, 3), Some(&HitTarget::Commit(0)));

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
            .nth(1)
            .unwrap()
            .contains("Merge branch 'develop'")
    );
    assert_eq!(hits.hit(100, 1), Some(&HitTarget::Commit(2)));
}

#[test]
fn selected_row_is_highlighted() {
    let repo = FixtureRepo::sample();
    let loaded = load(&repo);
    let mut v = view(&loaded);
    v.selected = 2;
    let (buf, _) = render(&v, 110, 10);
    let theme = Theme::default();
    // Display row 2 is on line 5 (header, then two lines per row).
    assert_eq!(buf[(105, 5)].bg, theme.selected_bg);
    assert_ne!(buf[(105, 6)].bg, theme.selected_bg);
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

    // Header to row 1, three blank lines, then row 1's connector onwards.
    assert_eq!(lines[..4], plain_lines[..4]);
    assert!(lines[4..7].iter().all(String::is_empty), "{lines:#?}");
    assert_eq!(lines[7..], plain_lines[4..7]);
    assert_eq!(hits.gap(), Some(Rect::new(0, 4, 100, 3)));
    // Clicks below the gap resolve to the shifted row (display 2 is commit 1).
    assert_eq!(hits.hit(40, 8), Some(&HitTarget::Commit(1)));
    assert_eq!(hits.hit(40, 5), None);
}

#[test]
fn a_gap_is_clipped_at_the_bottom_and_skipped_when_its_row_is_off_screen() {
    let repo = FixtureRepo::sample();
    let loaded = load(&repo);
    let clipped = GraphView {
        gap: Some(Gap {
            after: 1,
            height: 10,
        }),
        ..view(&loaded)
    };
    let (_, hits) = render(&clipped, 100, 6);
    assert_eq!(hits.gap(), Some(Rect::new(0, 4, 100, 2)));

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

#[test]
fn header_dividers_separate_the_columns_and_can_be_grabbed() {
    let repo = FixtureRepo::sample();
    let loaded = load(&repo);
    let (buf, hits) = render(&view(&loaded), 110, 10);
    // Graph 0..7, description 7..69, then date, author and hash, each after a divider.
    for (x, divider) in [
        (6, ColumnDivider::AfterGraph),
        (69, ColumnDivider::BeforeColumn(GraphColumn::Date)),
        (
            86,
            ColumnDivider::Between(GraphColumn::Date, GraphColumn::Author),
        ),
        (
            101,
            ColumnDivider::Between(GraphColumn::Author, GraphColumn::Commit),
        ),
    ] {
        assert_eq!(buf[(x, 0)].symbol(), "│", "x={x}");
        assert_eq!(hits.divider_at(x, 0), Some(divider), "x={x}");
    }
    assert_eq!(
        hits.divider_at(6, 1),
        None,
        "only the header line has dividers"
    );
    assert_eq!(hits.divider_at(7, 0), None);
    assert_eq!(hits.graph_width(), 7);
}

#[test]
fn column_widths_move_the_columns() {
    let repo = FixtureRepo::sample();
    let loaded = load(&repo);
    let wide = GraphView {
        widths: ColumnWidths {
            graph: Some(12),
            date: 20,
            ..ColumnWidths::default()
        },
        ..view(&loaded)
    };
    let (buf, hits) = render(&wide, 110, 10);
    assert_eq!(hits.divider_at(11, 0), Some(ColumnDivider::AfterGraph));
    // The date column is 20 wide, so it starts 4 columns further left.
    assert_eq!(
        hits.divider_at(65, 0),
        Some(ColumnDivider::BeforeColumn(GraphColumn::Date))
    );
    let line: String = (66..86).map(|x| buf[(x, 3)].symbol()).collect();
    assert!(line.starts_with("2026-01-01 00:07"), "{line:?}");
}

#[test]
fn hidden_columns_drop_their_header_and_divider_and_the_description_takes_the_room() {
    let repo = FixtureRepo::sample();
    let loaded = load(&repo);
    let hidden = [GraphColumn::Author];
    let without_author = GraphView {
        hidden: &hidden,
        ..view(&loaded)
    };
    let (buf, hits) = render(&without_author, 110, 10);
    let header = text(&buf).lines().next().unwrap().to_owned();
    assert!(!header.contains("Author"), "{header:?}");
    assert!(header.contains("│Date"), "{header:?}");
    assert!(header.contains("Commit"), "{header:?}");
    // 15 columns (author and its space) go to the description.
    assert_eq!(
        hits.divider_at(84, 0),
        Some(ColumnDivider::BeforeColumn(GraphColumn::Date))
    );
    assert_eq!(
        (0..110)
            .filter_map(|x| hits.divider_at(x, 0))
            .collect::<Vec<_>>(),
        [
            ColumnDivider::AfterGraph,
            ColumnDivider::BeforeColumn(GraphColumn::Date),
            ColumnDivider::Between(GraphColumn::Date, GraphColumn::Commit),
        ],
        "the author's divider is gone; date and commit trade width"
    );
    assert_eq!(buf[(101, 0)].symbol(), "│");
    let row = text(&buf).lines().nth(3).unwrap().to_owned();
    assert!(!row.contains("Fixture Author"), "no author names: {row:?}");

    let all = [GraphColumn::Date, GraphColumn::Author, GraphColumn::Commit];
    let bare = GraphView {
        hidden: &all,
        ..view(&loaded)
    };
    let (buf, hits) = render(&bare, 110, 10);
    let header = text(&buf).lines().next().unwrap().to_owned();
    assert!(header.ends_with("Description"), "{header:?}");
    assert_eq!(
        (0..110)
            .filter_map(|x| hits.divider_at(x, 0))
            .collect::<Vec<_>>(),
        [ColumnDivider::AfterGraph]
    );

    // Without the date, the author's left divider resizes the author.
    let no_date = [GraphColumn::Date];
    let (buf, hits) = render(
        &GraphView {
            hidden: &no_date,
            ..view(&loaded)
        },
        110,
        10,
    );
    let header = text(&buf).lines().next().unwrap().to_owned();
    let x = header.find("│Author").expect("a divider before Author");
    let x = u16::try_from(header[..x].chars().count()).unwrap();
    assert_eq!(
        hits.divider_at(x, 0),
        Some(ColumnDivider::BeforeColumn(GraphColumn::Author))
    );
}

#[test]
fn the_header_line_is_known_to_the_hit_map() {
    let repo = FixtureRepo::sample();
    let loaded = load(&repo);
    let (_, hits) = render(&view(&loaded), 110, 10);
    assert!(hits.is_header(30, 0));
    assert!(hits.is_header(109, 0));
    assert!(!hits.is_header(30, 1));
    assert!(!hits.is_header(110, 0));
}

#[test]
fn the_current_branch_has_a_circle_and_a_bold_name() {
    let repo = FixtureRepo::sample();
    let loaded = load(&repo);
    let (buf, _) = render(&view(&loaded), 110, 16);

    // HEAD is on main: a circle before its badge, and its name in bold.
    let (x, y) = find(&buf, " main ");
    assert_eq!(buf[(x - 2, y)].symbol(), "○");
    assert!(
        modifiers(&buf, x + 1, y, 4)
            .iter()
            .all(|m| m.contains(Modifier::BOLD))
    );
    // Other branches keep normal text and no circle.
    let (x, y) = find(&buf, " develop ");
    assert_ne!(buf[(x - 2, y)].symbol(), "○");
    assert!(
        modifiers(&buf, x + 1, y, 7)
            .iter()
            .all(|m| !m.contains(Modifier::BOLD))
    );
}

#[test]
fn remote_names_on_a_badge_are_italic() {
    let repo = FixtureRepo::sample();
    let loaded = load(&repo);
    let (buf, _) = render(&view(&loaded), 110, 16);
    let (x, y) = find(&buf, "│ origin ");
    assert!(
        modifiers(&buf, x + 2, y, 6)
            .iter()
            .all(|m| m.contains(Modifier::ITALIC))
    );
}

#[test]
fn nerd_badges_have_an_icon_and_rounded_ends() {
    let repo = FixtureRepo::sample();
    let loaded = load(&repo);
    let theme = Theme {
        icons: Icons::Nerd,
        ..Theme::default()
    };
    let (buf, hits) = render_with(&view(&loaded), 110, 16, &theme);
    let screen = text(&buf);
    // Codicons git-branch and tag, between Powerline half circles.
    assert!(
        screen.contains("○ \u{e0b6}\u{ec6f}  main │ origin \u{e0b4}"),
        "{screen}"
    );
    assert!(
        screen.contains("\u{e0b6}\u{ec6f}  develop \u{e0b4}"),
        "{screen}"
    );
    assert!(
        screen.contains("\u{e0b6}\u{ea66}  v0.1.0 \u{e0b4}"),
        "{screen}"
    );

    // The rounded ends belong to the badge: clicking one opens its menu.
    let (x, y) = find(&buf, "\u{e0b6}\u{ec6f}  develop");
    match hits.hit(x, y) {
        Some(HitTarget::Ref { full_name, .. }) => assert_eq!(full_name, "refs/heads/develop"),
        other => panic!("expected the develop badge, got {other:?}"),
    }
}
