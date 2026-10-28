//! Test support: drives the app state with real git through the runner.

use std::time::Duration;

use crossbeam_channel::Receiver;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tgg_testkit::FixtureRepo;

use crate::git::{JobResult, ProcessExecutor, Runner};
use crate::state::{AppState, Command, DEFAULT_PAGE_SIZE, Event};

/// Runs the app's git requests for real until nothing is pending.
pub struct Harness {
    pub state: AppState,
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
        };
        let commands = h.state.update(Event::Refresh);
        h.settle(commands);
        h
    }

    pub fn settle(&mut self, mut commands: Vec<Command>) {
        loop {
            for command in commands.drain(..) {
                let Command::Git(request) = command;
                let job = self.runner.submit(request.command(&self.state.repo));
                self.state.track(&job, request);
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
}
