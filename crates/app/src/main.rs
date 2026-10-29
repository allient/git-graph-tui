//! `tui-git-graph`: a Git Graph for the terminal.

mod browser;
mod cli;
mod clipboard;
mod config;
mod git;
mod panels;
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
use tgg_ui::{Icons, Theme};

use crate::cli::Cli;
use crate::git::{ProcessExecutor, Runner, discover_repo};
use crate::state::{AppState, Command, Event};

/// How long to wait for input before checking for git results.
const INPUT_POLL: Duration = Duration::from_millis(50);

fn main() -> Result<()> {
    let cli = Cli::parse();
    let env = |name: &str| std::env::var(name).ok();
    let config_path = config::path(&env);
    let config = match &config_path {
        Some(path) => config::load(path)?,
        None => config::Config::default(),
    };
    let icons = cli.icons(&env, config.icons)?;
    // Resolve the repository before touching the terminal, so errors print normally.
    let repo = discover_repo(&ProcessExecutor::new(), &cli.path)
        .map_err(|err| anyhow::anyhow!(err.user_message()))
        .with_context(|| format!("cannot open {}", cli.path.display()))?;
    let layout = ratatui::run(|terminal| run(terminal, repo, icons, config.layout))?;
    // Remember resized panes and columns for next time.
    if let Some(path) = config_path.filter(|_| layout != config.layout) {
        save_layout(&path, layout);
    }
    Ok(())
}

/// Saves the layout after the terminal is restored. A failure doesn't fail
/// the run: it is only reported.
#[expect(
    clippy::print_stderr,
    reason = "the terminal is restored and the app is exiting; stderr is all that is left"
)]
fn save_layout(path: &std::path::Path, layout: config::Layout) {
    if let Err(err) = config::save_layout(path, layout) {
        eprintln!("tui-git-graph: could not save the layout: {err:#}");
    }
}

/// Runs the app until it quits and returns the final pane and column widths.
fn run(
    terminal: &mut DefaultTerminal,
    repo: PathBuf,
    icons: Icons,
    layout: config::Layout,
) -> Result<config::Layout> {
    let _mouse = MouseCapture::enable()?;
    let (runner, results) = Runner::new(ProcessExecutor::new());
    let theme = Theme {
        icons,
        ..Theme::default()
    };
    let mut state = AppState::new(repo);
    state.apply_layout(layout);
    dispatch(&mut state, &runner, Event::Refresh);

    loop {
        terminal.draw(|frame| {
            view::draw(frame, &mut state, &theme);
        })?;
        if state.should_quit {
            return Ok(state.layout());
        }
        if event::poll(INPUT_POLL)? {
            match event::read()? {
                TermEvent::Key(key) => dispatch(&mut state, &runner, Event::Key(key)),
                TermEvent::Mouse(mouse) => dispatch(&mut state, &runner, Event::Mouse(mouse)),
                _ => {}
            }
        }
        while let Ok(result) = results.try_recv() {
            dispatch(&mut state, &runner, Event::Git(result));
        }
    }
}

/// Applies an event and starts the git requests it produces.
fn dispatch(state: &mut AppState, runner: &Runner<ProcessExecutor>, event: Event) {
    for command in state.update(event) {
        match command {
            Command::Git(request) => {
                let job = runner.submit(request.command(&state.repo));
                state.track(&job, request);
            }
            Command::Run(git) => {
                let job = runner.submit(git);
                state.track_action(&job);
            }
            Command::Clipboard(text) => {
                if let Err(err) = clipboard::copy(&text) {
                    state.message = Some(format!("Could not copy to the clipboard: {err}"));
                }
            }
            Command::OpenUrl(url) => {
                if let Err(err) = browser::open(&url) {
                    state.message = Some(format!("Could not open {url}: {err}"));
                }
            }
        }
    }
}

/// Turns on mouse reporting for the session and turns it off again on drop,
/// so the shell gets normal mouse behavior back even on an early return.
struct MouseCapture;

impl MouseCapture {
    fn enable() -> Result<Self> {
        crossterm::execute!(std::io::stdout(), event::EnableMouseCapture)?;
        Ok(Self)
    }
}

impl Drop for MouseCapture {
    fn drop(&mut self) {
        // Nothing useful to do if the terminal is already gone.
        let _ = crossterm::execute!(std::io::stdout(), event::DisableMouseCapture);
    }
}
