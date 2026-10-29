//! Test support: drives the app state with real git through the runner.

use std::time::Duration;

use crossbeam_channel::Receiver;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tgg_testkit::FixtureRepo;

use crate::external::External;
use crate::git::{JobResult, ProcessExecutor, Runner};
use crate::state::{AppState, Command, DEFAULT_PAGE_SIZE, Event};

/// Runs the app's git requests for real until nothing is pending.
pub struct Harness {
    pub state: AppState,
    pub screen: ratatui::buffer::Buffer,
    /// Texts the app copied to the clipboard.
    pub clipboard: Vec<String>,
    /// Pages the app asked to open in the browser.
    pub opened: Vec<String>,
    /// Programs the app asked to run with the terminal (never run here).
    pub external: Vec<External>,
    pub runner: Runner<ProcessExecutor>,
    pub results: Receiver<JobResult>,
}

impl Harness {
    pub fn open(repo: &FixtureRepo) -> Self {
        Self::with_page_size(repo, DEFAULT_PAGE_SIZE)
    }

    pub fn with_page_size(repo: &FixtureRepo, page_size: usize) -> Self {
        let (runner, results) = Runner::new(ProcessExecutor::new());
        let mut state = AppState::new(repo.path().to_path_buf());
        state.page_size = page_size;
        state.viewport_height = 5;
        let mut h = Self {
            state,
            runner,
            results,
            screen: ratatui::buffer::Buffer::empty(ratatui::layout::Rect::default()),
            clipboard: Vec::new(),
            opened: Vec::new(),
            external: Vec::new(),
        };
        let commands = h.state.update(Event::Refresh);
        h.settle(commands);
        h
    }

    pub fn settle(&mut self, mut commands: Vec<Command>) {
        loop {
            for command in commands.drain(..) {
                match command {
                    Command::Git(request) => {
                        let job = self.runner.submit(request.command(&self.state.repo));
                        self.state.track(&job, request);
                    }
                    Command::Run(git) => {
                        let job = self.runner.submit(git);
                        self.state.track_action(&job);
                    }
                    Command::Clipboard(text) => self.clipboard.push(text),
                    Command::OpenUrl(url) => self.opened.push(url),
                    Command::External(external) => self.external.push(external),
                }
            }
            if !self.state.is_loading() {
                return;
            }
            let result = self
                .results
                .recv_timeout(Duration::from_secs(10))
                .expect("git result");
            commands = self.state.update(Event::Git(result));
        }
    }

    pub fn key(&mut self, code: KeyCode) {
        let commands = self
            .state
            .update(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
        self.settle(commands);
    }

    pub fn selected_subject(&self) -> Option<&str> {
        self.state.selected_commit().map(|c| c.subject.as_str())
    }

    /// Renders the screen, which also refreshes the hit-maps clicks resolve against.
    pub fn draw(&mut self, width: u16, height: u16) {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|f| crate::view::draw(f, &mut self.state, &tgg_ui::Theme::default()))
            .unwrap();
        self.screen = terminal.backend().buffer().clone();
    }

    /// Position of the first cell where `text` appears in the last frame.
    pub fn find_on_screen(&self, text: &str) -> Option<(u16, u16)> {
        let area = self.screen.area;
        (area.top()..area.bottom()).find_map(|y| {
            let cells: Vec<&str> = (area.left()..area.right())
                .map(|x| self.screen[(x, y)].symbol())
                .collect();
            let line: String = cells.concat();
            let byte = line.find(text)?;
            let x = line[..byte].chars().count();
            Some((u16::try_from(x).ok()?, y))
        })
    }
}
