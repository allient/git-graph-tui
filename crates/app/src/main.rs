//! `tui-git-graph`: a Git Graph for the terminal.

mod cli;
mod git;
mod state;
#[cfg(test)]
mod testing;
mod view;

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use crossterm::event::{self, Event as TermEvent};
use ratatui::DefaultTerminal;
use tgg_ui::Theme;

use crate::cli::Cli;
use crate::git::{ProcessExecutor, Runner, discover_repo};
use crate::state::{AppState, Command, Event};

/// How long to wait for input before checking for git results.
const INPUT_POLL: Duration = Duration::from_millis(50);

fn main() -> Result<()> {
    let cli = Cli::parse();
    // Resolve the repository before touching the terminal, so errors print normally.
    let repo = discover_repo(&ProcessExecutor::new(), &cli.path)
        .map_err(|err| anyhow::anyhow!(err.user_message()))
        .with_context(|| format!("cannot open {}", cli.path.display()))?;
    ratatui::run(|terminal| run(terminal, repo))
}

fn run(terminal: &mut DefaultTerminal, repo: PathBuf) -> Result<()> {
    let (runner, results) = Runner::new(ProcessExecutor::new());
    let theme = Theme::default();
    let mut state = AppState::new(repo);
    dispatch(&mut state, &runner, Event::Refresh);

    loop {
        terminal.draw(|frame| {
            view::draw(frame, &mut state, &theme);
        })?;
        if state.should_quit {
            return Ok(());
        }
        if event::poll(INPUT_POLL)?
            && let TermEvent::Key(key) = event::read()?
        {
            dispatch(&mut state, &runner, Event::Key(key));
        }
        while let Ok(result) = results.try_recv() {
            dispatch(&mut state, &runner, Event::Git(result));
        }
    }
}

/// Applies an event and starts the git requests it produces.
fn dispatch(state: &mut AppState, runner: &Runner<ProcessExecutor>, event: Event) {
    for Command::Git(request) in state.update(event) {
        let job = runner.submit(request.command(&state.repo));
        state.track(&job, request);
    }
}
