//! `tui-git-graph`: a Git Graph for the terminal.

mod cli;

use anyhow::Result;
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

fn main() -> Result<()> {
    let cli = Cli::parse();
    ratatui::run(|terminal| run(terminal, &cli))
}

/// Placeholder screen until the graph view lands (see `odd/tasks/git-graph-mvp.md`).
fn run(terminal: &mut DefaultTerminal, cli: &Cli) -> Result<()> {
    let path = cli.path.display().to_string();
    loop {
        terminal.draw(|frame| {
            let [body, footer] =
                Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(frame.area());
            let text = Paragraph::new(vec![
                Line::from("tui-git-graph".bold()),
                Line::from(format!("Repository path: {path}")),
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
