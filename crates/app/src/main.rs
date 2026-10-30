//! `git-graph-tui`: a Git Graph for the terminal.

mod browser;
mod cli;
mod clipboard;
mod config;
mod external;
mod git;
mod herdr;
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
use ggt_ui::{Icons, Theme};
use ratatui::DefaultTerminal;

use crate::cli::Cli;
use crate::external::External;
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
    let pager = external::pager(&env, config.diff.pager);
    let editor = external::editor(&env);
    let issues = config.issues;
    // Resolve the repository before touching the terminal, so errors print normally.
    let repo = discover_repo(&ProcessExecutor::new(), &cli.path)
        .map_err(|err| anyhow::anyhow!(err.user_message()))
        .with_context(|| format!("cannot open {}", cli.path.display()))?;
    let (before, after) = ratatui::run(|terminal| {
        let programs = Programs { pager, editor };
        run(terminal, repo, icons, config.layout, programs, issues)
    })?;
    // Remember resized panes and columns for next time.
    if let Some(path) = config_path.filter(|_| after != before) {
        save_layout(&path, after);
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
        eprintln!("git-graph-tui: could not save the layout: {err:#}");
    }
}

/// The programs the app hands the terminal to.
struct Programs {
    /// Diff pager (S12); None uses the built-in diff view.
    pager: Option<String>,
    /// Editor for "Open File" in the Source Control pane.
    editor: String,
}

/// Runs the app until it quits. Returns the pane and column widths it started
/// with and those it ended with.
fn run(
    terminal: &mut DefaultTerminal,
    repo: PathBuf,
    icons: Icons,
    layout: config::Layout,
    programs: Programs,
    issues: config::Issues,
) -> Result<(config::Layout, config::Layout)> {
    let _mouse = MouseCapture::enable()?;
    // Inside herdr, plain right-click opens the app's menus, not herdr's.
    let env = |name: &str| std::env::var(name).ok();
    let _right_click = herdr::RightClick::enable(&env, herdr::run_quietly);
    let (runner, results) = Runner::new(ProcessExecutor::new());
    let theme = Theme {
        icons,
        ..Theme::default()
    };
    let mut state = AppState::new(repo);
    state.pager = programs.pager;
    state.editor = programs.editor;
    state.issues = issues;
    let before = state.apply_layout(layout);
    dispatch(&mut state, &runner, Event::Refresh);

    loop {
        terminal.draw(|frame| {
            view::draw(frame, &mut state, &theme);
        })?;
        if state.should_quit {
            return Ok((before, state.layout()));
        }
        let mut external = Vec::new();
        if event::poll(INPUT_POLL)? {
            match event::read()? {
                TermEvent::Key(key) => external = dispatch(&mut state, &runner, Event::Key(key)),
                TermEvent::Mouse(mouse) => {
                    external = dispatch(&mut state, &runner, Event::Mouse(mouse));
                }
                _ => {}
            }
        }
        for program in external {
            if let Some(message) = hand_over(terminal, &program)? {
                state.message = Some(message);
            }
            // An editor may have changed files: show them as they are now.
            if program.refresh {
                dispatch(&mut state, &runner, Event::Refresh);
            }
        }
        while let Ok(result) = results.try_recv() {
            dispatch(&mut state, &runner, Event::Git(result));
        }
    }
}

/// Steps the TUI aside, runs `program` in the plain terminal and comes back
/// with a full redraw. Returns the status message when the program failed.
/// Not unit-tested (it needs a real terminal); `external::run` is.
fn hand_over(terminal: &mut DefaultTerminal, program: &External) -> Result<Option<String>> {
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        std::io::stdout(),
        event::DisableMouseCapture,
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::cursor::Show
    )?;
    let message = external::run(program);
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::EnterAlternateScreen,
        event::EnableMouseCapture
    )?;
    // Redraw everything on the next frame. `resize` clears the screen and the
    // back buffer without asking the terminal where its cursor is, which
    // `clear` does (and which fails when the terminal is slow to answer).
    let area = terminal.size()?.into();
    terminal.resize(area)?;
    Ok(message)
}

/// Applies an event and starts the git requests it produces. Returns the
/// programs to run with the terminal handed over, which only `run` can do.
fn dispatch(state: &mut AppState, runner: &Runner<ProcessExecutor>, event: Event) -> Vec<External> {
    let mut external = Vec::new();
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
            Command::External(program) => external.push(program),
        }
    }
    external
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
