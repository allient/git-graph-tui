//! `tui-git-graph`: a Git Graph for the terminal.

mod cli;
mod git;

use std::path::Path;

use anyhow::{Context, Result};
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    DefaultTerminal,
    layout::{Constraint, Layout},
    style::Stylize,
    text::Line,
    widgets::{Block, Paragraph},
};

use crate::cli::Cli;
use crate::git::{ProcessExecutor, discover_repo};

fn main() -> Result<()> {
    let cli = Cli::parse();
    // Resolve the repository before touching the terminal, so errors print normally.
    let repo = discover_repo(&ProcessExecutor::new(), &cli.path)
        .map_err(|err| anyhow::anyhow!(err.user_message()))
        .with_context(|| format!("cannot open {}", cli.path.display()))?;
    ratatui::run(|terminal| run(terminal, &repo))
}

/// Placeholder screen until the graph view lands (see `odd/tasks/git-graph-mvp.md`).
fn run(terminal: &mut DefaultTerminal, repo: &Path) -> Result<()> {
    let path = repo.display().to_string();
    loop {
        terminal.draw(|frame| {
            let [body, footer] =
                Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(frame.area());
            let text = Paragraph::new(vec![
                Line::from("tui-git-graph".bold()),
                Line::from(format!("Repository: {path}")),
                Line::from("The graph view is not implemented yet.".dim()),
            ])
            .block(Block::bordered().title(" Git Graph "));
            frame.render_widget(text, body);
            frame.render_widget(Line::from(" q quit".dim()), footer);
        })?;

        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
        {
            return Ok(());
        }
    }
}
