//! Draws the whole screen from `AppState`.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Widget};
use tgg_model::HeadState;
use tgg_ui::{BannerHitMap, Gap, GraphView, SourceControlHitMap, Theme};

use crate::state::{AppState, Dialog, DialogHits, Find, MIN_WIDTH_FOR_PANE};

/// Key hints shown at the right of the status bar.
const HINTS: &str =
    "j/k move  enter details  m menu  / find  c commit  tab toolbar  H head  r refresh  q quit";

/// Most lines the inline details panel takes.
const DETAILS_HEIGHT: u16 = 12;

/// Graph lines kept visible above and below an open details panel.
const MIN_GRAPH_LINES: u16 = 3;

/// The full-area panels drawn over the graph: git output, worktrees, reflog
/// and interactive rebase. Each records the area its rows took.
fn draw_panels(frame: &mut Frame<'_>, graph_area: Rect, state: &mut AppState, theme: &Theme) {
    if let Some(view) = &mut state.output_view {
        view.page =
            crate::panels::draw_output(frame, graph_area, &state.git_output, view.offset, theme);
    }
    if let Some(view) = &mut state.worktrees {
        view.area = crate::panels::draw_worktrees(frame, graph_area, view, theme);
    }
    if let Some(view) = &mut state.reflog {
        view.area = crate::panels::draw_reflog(frame, graph_area, view, theme);
    }
    if let Some(view) = &mut state.rebase {
        view.area = crate::panels::draw_rebase(frame, graph_area, view, theme);
    }
}

/// Narrowest the Source Control pane gets when dragged.
const MIN_PANE_WIDTH: u16 = 20;
/// Columns always left for the graph beside the pane.
const MIN_GRAPH_AREA: u16 = 50;

/// Width of the Source Control pane: the dragged width, or a quarter of the
/// terminal; within bounds either way.
fn pane_width(total: u16, chosen: Option<u16>) -> u16 {
    chosen.unwrap_or((total / 4).clamp(30, 44)).clamp(
        MIN_PANE_WIDTH,
        total.saturating_sub(MIN_GRAPH_AREA).max(MIN_PANE_WIDTH),
    )
}

/// Draws the Source Control pane and, beside it, the toolbar and the graph;
/// then the status bar and whatever is open on top. Updates the viewport
/// height and stores the hit-maps that mouse events resolve against.
pub fn draw(frame: &mut Frame<'_>, state: &mut AppState, theme: &Theme) {
    let [main_area, status_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(frame.area());
    let right_area = draw_source_control(frame, main_area, state, theme);
    state.toolbar.more = !state.source_control.visible;
    let banner = state.banner();
    let banner_height = u16::from(banner.is_some());
    let [toolbar_area, banner_area, graph_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(banner_height),
        Constraint::Fill(1),
    ])
    .areas(right_area);
    state.toolbar.hits = state
        .toolbar
        .widget()
        .render(toolbar_area, frame.buffer_mut(), theme);
    state.banner_hits = banner.map_or_else(BannerHitMap::default, |b| {
        b.render(banner_area, frame.buffer_mut(), theme)
    });
    let gap = state.details_display_index().map(|after| Gap {
        after,
        height: DETAILS_HEIGHT.min(graph_area.height.saturating_sub(MIN_GRAPH_LINES)),
    });
    let gap_height = gap.map_or(0, |g| g.height);
    state.viewport_height = GraphView::rows_fitting(graph_area.height - gap_height);

    let refs = state.visible_refs();
    let view = GraphView {
        commits: &state.commits,
        rows: state.graph.rows(),
        refs: &refs,
        head: state.head.as_ref(),
        uncommitted: state.status.changed_files(),
        offset: state.offset,
        selected: state.selected,
        gap,
        widths: state.columns,
        matches: state.find.as_ref().map_or(&[], |f| f.matches.as_slice()),
        hidden: &state.hidden_columns,
    };
    state.graph_hits = view.render(graph_area, frame.buffer_mut(), theme);
    if let (Some(area), Some(details)) = (state.graph_hits.gap(), &mut state.details) {
        details.hits = if let Some(view) = &details.view {
            Some(view.render(area, frame.buffer_mut(), theme))
        } else {
            Clear.render(area, frame.buffer_mut());
            let loading = if details.compare.is_some() {
                " Loading the comparison…"
            } else {
                " Loading commit details…"
            };
            Paragraph::new(loading.dim()).render(area, frame.buffer_mut());
            None
        };
    }
    draw_status(frame, status_area, state, theme);
    let screen = frame.area();
    if let Some(diff) = &mut state.diff {
        Clear.render(graph_area, frame.buffer_mut());
        let block = Block::bordered().title(" diff · esc closes ");
        let inner = block.inner(graph_area);
        block.render(graph_area, frame.buffer_mut());
        diff.hits = Some(diff.view.render(inner, frame.buffer_mut(), theme));
    }
    draw_panels(frame, graph_area, state, theme);
    if let Some(menu) = &state.menu {
        state.menu_hits = Some(menu.render(screen, frame.buffer_mut(), theme));
    }
    if let Some(picker) = &mut state.picker {
        let hits = picker.view.render(screen, frame.buffer_mut(), theme);
        if let Some(cursor) = hits.cursor() {
            frame.set_cursor_position(cursor);
        }
        picker.hits = Some(hits);
    }
    if let Some(info) = &state.info {
        crate::panels::draw_info(frame, screen, info, theme);
    }
    match &state.dialog {
        Some(Dialog::Input { view, .. }) => {
            let hits = view.render(screen, frame.buffer_mut(), theme);
            if let Some(cursor) = hits.cursor() {
                frame.set_cursor_position(cursor);
            }
            state.dialog_hits = Some(DialogHits::Input(hits));
        }
        Some(Dialog::Confirm { view, .. }) => {
            let hits = view.render(screen, frame.buffer_mut(), theme);
            state.dialog_hits = Some(DialogHits::Confirm(hits));
        }
        None => {}
    }
}

/// Draws the pane and a divider when the terminal is wide enough, and returns
/// the area left for the graph.
fn draw_source_control(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &mut AppState,
    theme: &Theme,
) -> Rect {
    state.source_control.visible = area.width >= MIN_WIDTH_FOR_PANE;
    if !state.source_control.visible {
        state.source_control.divider = None;
        state.source_control.focus = false;
        state.source_control.hits = SourceControlHitMap::default();
        return area;
    }
    let width = pane_width(area.width, state.source_control.width);
    if state.source_control.width.is_some() {
        state.source_control.width = Some(width);
    }
    let [pane, divider, rest] = Layout::horizontal([
        Constraint::Length(width),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(area);
    let hits = state.source_control.view.render(
        pane,
        frame.buffer_mut(),
        theme,
        state.source_control.focus,
    );
    if let Some(cursor) = hits.cursor() {
        frame.set_cursor_position(cursor);
    }
    state.source_control.hits = hits;
    state.source_control.divider = Some(divider.x);
    for y in divider.top()..divider.bottom() {
        frame.buffer_mut()[(divider.x, y)]
            .set_char('│')
            .set_fg(theme.muted);
    }
    rest
}

/// The find bar, on the status line while find is open.
fn draw_find(frame: &mut Frame<'_>, area: Rect, find: &Find, theme: &Theme) {
    let label = " Find: ";
    let count = match (find.query.text().is_empty(), find.matches.len()) {
        (true, _) => String::new(),
        _ if find.searching.is_some() => "Searching older history…".to_owned(),
        (false, 0) => "No matches".to_owned(),
        (false, n) => format!("{} of {n}", find.current + 1),
    };
    let line = Line::from(vec![
        Span::styled(label, Style::new().add_modifier(Modifier::BOLD)),
        Span::raw(find.query.text().to_owned()),
        Span::raw("  "),
        Span::styled(count, Style::new().fg(theme.tag)),
    ]);
    frame.render_widget(line, area);
    frame.render_widget(
        Line::from("enter next  ↑ previous  esc close ".dim()).right_aligned(),
        area,
    );
    let cursor = label.len() + find.query.width_before_cursor();
    frame.set_cursor_position((area.x + u16::try_from(cursor).unwrap_or(u16::MAX), area.y));
}

fn draw_status(frame: &mut Frame<'_>, area: Rect, state: &AppState, theme: &Theme) {
    if let Some(find) = &state.find {
        draw_find(frame, area, find, theme);
        return;
    }
    let branch = match &state.status.head {
        HeadState::Branch(name) => name.clone(),
        HeadState::Detached => "detached HEAD".to_owned(),
        HeadState::Unborn => "no commits yet".to_owned(),
    };
    let repo = state.repo.file_name().map_or_else(
        || state.repo.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );

    let mut spans = vec![
        Span::styled(
            format!(" {repo} "),
            Style::new().add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {branch} "), Style::new().fg(theme.lane(0))),
    ];
    if state.is_loading() {
        spans.push(Span::styled(" loading… ", Style::new().fg(theme.muted)));
    }
    if let Some(message) = &state.message {
        spans.push(Span::styled(
            format!(" {message} "),
            Style::new().fg(theme.lane(5)),
        ));
    }
    let left = Line::from(spans);
    let room_for_hints = usize::from(area.width) > left.width() + HINTS.len() + 2;
    frame.render_widget(left, area);
    if room_for_hints {
        frame.render_widget(Line::from(HINTS.dim()).right_aligned(), area);
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use tgg_testkit::FixtureRepo;
    use tgg_ui::HitTarget;

    use super::*;
    use crate::testing::Harness;

    fn screen(terminal: &Terminal<TestBackend>) -> String {
        let buf = terminal.backend().buffer();
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn whole_screen_of_the_sample_repo() {
        let repo = FixtureRepo::sample();
        let mut h = Harness::open(&repo);
        let mut terminal = Terminal::new(TestBackend::new(110, 10)).unwrap();
        terminal
            .draw(|f| {
                draw(f, &mut h.state, &Theme::default());
            })
            .unwrap();
        let text = screen(&terminal);
        let last = text.lines().last().unwrap();
        assert!(last.contains(" main "), "{last}");
        assert!(last.contains("q quit"), "{last}");
        // The repo name is a random temp dir; snapshot everything above the status bar.
        let graph: Vec<&str> = text.lines().take(9).collect();
        insta::assert_snapshot!(graph.join("\n"));
    }

    #[test]
    fn viewport_height_follows_the_terminal() {
        let repo = FixtureRepo::sample();
        let mut h = Harness::open(&repo);
        let mut terminal = Terminal::new(TestBackend::new(80, 6)).unwrap();
        terminal
            .draw(|f| {
                draw(f, &mut h.state, &Theme::default());
            })
            .unwrap();
        assert_eq!(
            h.state.viewport_height, 2,
            "the toolbar, status bar and column header take a line each; rows take two"
        );
    }

    #[test]
    fn hit_map_reflects_scrolling() {
        let repo = FixtureRepo::sample();
        let mut h = Harness::open(&repo);
        let mut terminal = Terminal::new(TestBackend::new(80, 4)).unwrap();
        terminal
            .draw(|f| {
                draw(f, &mut h.state, &Theme::default());
            })
            .unwrap();
        h.key(crossterm::event::KeyCode::Char('G'));
        terminal
            .draw(|f| draw(f, &mut h.state, &Theme::default()))
            .unwrap();
        // Toolbar, 2 graph lines, status bar; the last commit (index 6) is on the bottom graph line.
        assert_eq!(h.state.graph_hits.hit(40, 2), Some(&HitTarget::Commit(6)));
    }

    #[test]
    fn errors_show_in_the_status_bar() {
        let repo = FixtureRepo::sample();
        let mut h = Harness::open(&repo);
        h.state.message = Some("Authentication failed.".into());
        let mut terminal = Terminal::new(TestBackend::new(110, 5)).unwrap();
        terminal
            .draw(|f| {
                draw(f, &mut h.state, &Theme::default());
            })
            .unwrap();
        assert!(
            screen(&terminal)
                .lines()
                .last()
                .unwrap()
                .contains("Authentication failed.")
        );
    }
}
