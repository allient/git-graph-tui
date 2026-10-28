//! Draws the whole screen from `AppState`.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Widget};
use tgg_model::HeadState;
use tgg_ui::{Gap, GraphView, Theme};

use crate::state::{AppState, Dialog, DialogHits};

/// Key hints shown at the right of the status bar.
const HINTS: &str = "j/k move  enter details  m menu  H head  r refresh  q quit";

/// Most lines the inline details panel takes.
const DETAILS_HEIGHT: u16 = 12;

/// Graph lines kept visible above and below an open details panel.
const MIN_GRAPH_LINES: u16 = 3;

/// Draws the graph, the status bar and the open menu. Updates the viewport
/// height and stores the hit-maps that mouse events resolve against.
pub fn draw(frame: &mut Frame<'_>, state: &mut AppState, theme: &Theme) {
    let [graph_area, status_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(frame.area());
    let gap = state.details_display_index().map(|after| Gap {
        after,
        height: DETAILS_HEIGHT.min(graph_area.height.saturating_sub(MIN_GRAPH_LINES)),
    });
    let gap_height = gap.map_or(0, |g| g.height);
    state.viewport_height = usize::from(graph_area.height - gap_height).max(1);

    let view = GraphView {
        commits: &state.commits,
        rows: state.graph.rows(),
        refs: &state.refs,
        head: state.head.as_ref(),
        uncommitted: state.status.changed_files(),
        offset: state.offset,
        selected: state.selected,
        gap,
    };
    state.graph_hits = view.render(graph_area, frame.buffer_mut(), theme);
    if let (Some(area), Some(details)) = (state.graph_hits.gap(), &mut state.details) {
        details.hits = if let Some(view) = &details.view {
            Some(view.render(area, frame.buffer_mut(), theme))
        } else {
            Clear.render(area, frame.buffer_mut());
            Paragraph::new(" Loading commit details…".dim()).render(area, frame.buffer_mut());
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
    if let Some(view) = &mut state.output_view {
        view.page =
            crate::panels::draw_output(frame, graph_area, &state.git_output, view.offset, theme);
    }
    if let Some(menu) = &state.menu {
        state.menu_hits = Some(menu.render(screen, frame.buffer_mut(), theme));
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

fn draw_status(frame: &mut Frame<'_>, area: Rect, state: &AppState, theme: &Theme) {
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
        assert_eq!(h.state.viewport_height, 5, "one line is the status bar");
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
        // 3 graph lines; the last commit (index 6) is on the bottom one.
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
