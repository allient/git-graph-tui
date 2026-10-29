//! Full-screen and modal panels drawn over the graph: the info dialog, the
//! git output view, the reflog view, the worktrees view and the interactive
//! rebase panel.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Widget, Wrap};
use tgg_actions::TodoAction;
use tgg_ui::{Theme, fit, format_date};

use crate::state::{
    GitOutput, Info, OutputLine, RebaseView, ReflogView, WorktreeEntry, WorktreesView,
};

/// Widest the info dialog gets.
const INFO_WIDTH: u16 = 72;

/// The info dialog, centered on `screen`.
pub fn draw_info(frame: &mut Frame<'_>, screen: Rect, info: &Info, theme: &Theme) {
    let width = INFO_WIDTH.min(screen.width.saturating_sub(2)).max(1);
    // Wrapped lines can need more rows than lines; leave room for a few.
    let wanted = u16::try_from(info.lines.len())
        .unwrap_or(u16::MAX)
        .saturating_add(4);
    let height = wanted.min(screen.height.saturating_sub(2)).max(1);
    let area = Rect::new(
        screen.x + (screen.width - width) / 2,
        screen.y + (screen.height - height) / 2,
        width,
        height,
    );
    Clear.render(area, frame.buffer_mut());
    let block = Block::bordered()
        .border_style(Style::new().fg(theme.muted))
        .title(format!(" {} · esc closes ", info.title));
    let text: Vec<Line<'_>> = info.lines.iter().map(|l| Line::from(l.as_str())).collect();
    Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: false })
        .render(area, frame.buffer_mut());
}

/// The git output view over `area`; returns the rows it shows per page.
pub fn draw_output(
    frame: &mut Frame<'_>,
    area: Rect,
    output: &GitOutput,
    offset: usize,
    theme: &Theme,
) -> usize {
    Clear.render(area, frame.buffer_mut());
    let block = Block::bordered().title(" git output · esc closes ");
    let inner = block.inner(area);
    block.render(area, frame.buffer_mut());
    let page = usize::from(inner.height);
    let lines = output.lines();
    let offset = offset.min(lines.len().saturating_sub(page.max(1)));
    let text: Vec<Line<'_>> = lines
        .into_iter()
        .skip(offset)
        .take(page)
        .map(|line| match line {
            OutputLine::Command { command, status } => {
                let ok = status == "ok";
                let mark = if ok {
                    "✓".to_owned()
                } else {
                    format!("✗ {status}")
                };
                Line::from(vec![
                    Span::styled(
                        format!("$ {command}"),
                        Style::new().add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        mark,
                        Style::new().fg(if ok { theme.lane(2) } else { theme.lane(5) }),
                    ),
                ])
            }
            OutputLine::Stdout(text) => Line::from(format!("  {text}")),
            OutputLine::Stderr(text) => {
                Line::styled(format!("  {text}"), Style::new().fg(theme.muted))
            }
            OutputLine::Note(text) => Line::styled(text, Style::new().fg(theme.muted)),
        })
        .collect();
    Paragraph::new(text).render(inner, frame.buffer_mut());
    page
}

/// Short hash length in the reflog view, like the graph's.
const SHORT_HASH: usize = 7;

/// The reflog view over `area`, one entry per line: short hash, selector,
/// subject and date. Returns the rows' area, for clicks and paging.
pub fn draw_reflog(frame: &mut Frame<'_>, area: Rect, view: &ReflogView, theme: &Theme) -> Rect {
    Clear.render(area, frame.buffer_mut());
    let block = Block::bordered().title(" reflog · enter actions · esc closes ");
    let inner = block.inner(area);
    block.render(area, frame.buffer_mut());
    if view.entries.is_empty() {
        Paragraph::new(Line::styled(
            "The reflog is empty.",
            Style::new().fg(theme.muted),
        ))
        .render(inner, frame.buffer_mut());
        return inner;
    }
    let page = usize::from(inner.height).max(1);
    let offset = view.offset.min(view.entries.len().saturating_sub(page));
    let text: Vec<Line<'_>> = view
        .entries
        .iter()
        .enumerate()
        .skip(offset)
        .take(page)
        .map(|(index, entry)| {
            let hash = format!("{} ", entry.id.short(SHORT_HASH));
            let selector = format!("{:<11} ", entry.selector());
            let date = format!(" {}", format_date(entry.time));
            // The subject takes what's left, so the dates line up on the right.
            let used = hash.len() + selector.len() + date.len();
            let subject = fit(
                &entry.subject,
                usize::from(inner.width).saturating_sub(used),
            );
            let line = Line::from(vec![
                Span::styled(hash, Style::new().fg(theme.hash)),
                Span::styled(selector, Style::new().fg(theme.muted)),
                Span::styled(subject, Style::new().fg(theme.text)),
                Span::styled(date, Style::new().fg(theme.date)),
            ]);
            if index == view.selected {
                line.style(Style::new().bg(theme.selected_bg))
            } else {
                line
            }
        })
        .collect();
    Paragraph::new(text).render(inner, frame.buffer_mut());
    inner
}

/// Widest the worktrees view's branch column gets.
const BRANCH_WIDTH: usize = 30;

/// The worktrees view over `area`, one worktree per line: a `*` on the one
/// the app runs in, short hash, branch (or what HEAD is), folder and flags.
/// Returns the rows' area, for clicks and paging.
pub fn draw_worktrees(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &WorktreesView,
    theme: &Theme,
) -> Rect {
    Clear.render(area, frame.buffer_mut());
    let block = Block::bordered().title(" worktrees · enter actions · a add · esc closes ");
    let inner = block.inner(area);
    block.render(area, frame.buffer_mut());
    let page = usize::from(inner.height).max(1);
    let offset = view.offset.min(view.entries.len().saturating_sub(page));
    let branch_width = view
        .entries
        .iter()
        .map(|e| branch_label(e).chars().count())
        .max()
        .unwrap_or(0)
        .min(BRANCH_WIDTH);
    let text: Vec<Line<'_>> = view
        .entries
        .iter()
        .enumerate()
        .skip(offset)
        .take(page)
        .map(|(index, entry)| {
            let worktree = &entry.worktree;
            let mark = if entry.is_current { "* " } else { "  " };
            let hash = worktree.head.as_ref().map_or_else(
                || " ".repeat(SHORT_HASH + 1),
                |id| format!("{} ", id.short(SHORT_HASH)),
            );
            let branch = format!("{} ", fit(&branch_label(entry), branch_width));
            let flags = worktree_flags(entry);
            let flags = if flags.is_empty() {
                String::new()
            } else {
                format!(" {flags}")
            };
            // The folder takes what's left, so the flags line up on the right.
            let used = mark.len() + hash.len() + branch.chars().count() + flags.chars().count();
            let path = fit(
                &worktree.path,
                usize::from(inner.width).saturating_sub(used),
            );
            let line = Line::from(vec![
                Span::styled(mark, Style::new().fg(theme.text)),
                Span::styled(hash, Style::new().fg(theme.hash)),
                Span::styled(branch, Style::new().fg(theme.text)),
                Span::styled(path, Style::new().fg(theme.muted)),
                Span::styled(flags, Style::new().fg(theme.date)),
            ]);
            if index == view.selected {
                line.style(Style::new().bg(theme.selected_bg))
            } else {
                line
            }
        })
        .collect();
    Paragraph::new(text).render(inner, frame.buffer_mut());
    inner
}

/// The branch checked out in a worktree, or what its HEAD is instead.
fn branch_label(entry: &WorktreeEntry) -> String {
    let worktree = &entry.worktree;
    match &worktree.branch {
        Some(branch) => branch.clone(),
        None if worktree.bare => "(bare)".into(),
        None => "(detached)".into(),
    }
}

/// `current · main · locked (reason) · prunable`, as they apply.
fn worktree_flags(entry: &WorktreeEntry) -> String {
    let worktree = &entry.worktree;
    let locked = worktree.locked.as_ref().map(|reason| {
        if reason.is_empty() {
            "locked".to_owned()
        } else {
            format!("locked ({reason})")
        }
    });
    [
        entry.is_current.then(|| "current".to_owned()),
        worktree.is_main.then(|| "main".to_owned()),
        locked,
        worktree.prunable.as_ref().map(|_| "prunable".to_owned()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ")
}

/// Keys of the interactive rebase panel, after the preview count.
const REBASE_KEYS: &str = "p pick · s squash · f fixup · d drop · space cycle · J/K move";

/// The interactive rebase panel over `area`: one commit per line, oldest
/// first (git's todo order), with its action, short hash and subject; the
/// preview count and the keys on the last line. Returns the rows' area,
/// for clicks and paging.
pub fn draw_rebase(frame: &mut Frame<'_>, area: Rect, view: &RebaseView, theme: &Theme) -> Rect {
    Clear.render(area, frame.buffer_mut());
    let block = Block::bordered().title(format!(
        " interactive rebase onto {} · enter start · esc cancel ",
        view.rebase.base.short(SHORT_HASH)
    ));
    let inner = block.inner(area);
    block.render(area, frame.buffer_mut());
    let rows = Rect {
        height: inner.height.saturating_sub(1),
        ..inner
    };
    let footer = Rect {
        y: rows.bottom(),
        height: inner.height.min(1),
        ..inner
    };
    let page = usize::from(rows.height).max(1);
    let items = &view.rebase.items;
    let offset = view.offset.min(items.len().saturating_sub(page));
    let text: Vec<Line<'_>> = items
        .iter()
        .enumerate()
        .skip(offset)
        .take(page)
        .map(|(index, item)| {
            let (color, subject_style) = match item.action {
                TodoAction::Pick => (theme.text, Style::new().fg(theme.text)),
                TodoAction::Squash | TodoAction::Fixup => {
                    (theme.lane(3), Style::new().fg(theme.text))
                }
                TodoAction::Drop => (
                    theme.muted,
                    Style::new()
                        .fg(theme.muted)
                        .add_modifier(Modifier::CROSSED_OUT),
                ),
            };
            let action = format!("{:<7}", item.action.word());
            let hash = format!("{} ", item.id.short(SHORT_HASH));
            let used = action.len() + hash.len();
            let subject = fit(&item.subject, usize::from(rows.width).saturating_sub(used));
            let line = Line::from(vec![
                Span::styled(action, Style::new().fg(color).add_modifier(Modifier::BOLD)),
                Span::styled(hash, Style::new().fg(theme.hash)),
                Span::styled(subject, subject_style),
            ]);
            if index == view.selected {
                line.style(Style::new().bg(theme.selected_bg))
            } else {
                line
            }
        })
        .collect();
    Paragraph::new(text).render(rows, frame.buffer_mut());
    Paragraph::new(Line::styled(
        format!("{} · {REBASE_KEYS}", view.rebase.preview()),
        Style::new().fg(theme.muted),
    ))
    .render(footer, frame.buffer_mut());
    rows
}
