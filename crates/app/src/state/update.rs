//! The "Update available" panel and what its buttons do.

use crossterm::event::KeyCode;

use super::{AppState, Command, Level};

/// The release page `o` and "Release notes" open.
pub const RELEASES_URL: &str = "https://github.com/allient/git-graph-tui/releases/latest";

/// A button of the update panel, in the order it is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateButton {
    /// Run the upgrade command with the terminal handed over.
    Upgrade,
    /// Copy the upgrade command to the clipboard.
    Copy,
    /// Open the release notes in the browser.
    Notes,
    /// Don't mention this version again.
    Skip,
}

impl UpdateButton {
    pub fn label(self) -> &'static str {
        match self {
            Self::Upgrade => "Upgrade now (u)",
            Self::Copy => "Copy command (c)",
            Self::Notes => "Release notes (o)",
            Self::Skip => "Skip this version (d)",
        }
    }

    fn for_key(code: KeyCode) -> Option<Self> {
        match code {
            KeyCode::Char('u') => Some(Self::Upgrade),
            KeyCode::Char('c') => Some(Self::Copy),
            KeyCode::Char('o') => Some(Self::Notes),
            KeyCode::Char('d') => Some(Self::Skip),
            _ => None,
        }
    }
}

impl AppState {
    /// The buttons the update panel offers for this install.
    pub fn update_panel_buttons(&self) -> Vec<UpdateButton> {
        let mut buttons = Vec::new();
        if self.install.upgrades_in_place() {
            buttons.push(UpdateButton::Upgrade);
        }
        if self.install.command().is_some() {
            buttons.push(UpdateButton::Copy);
        }
        buttons.extend([UpdateButton::Notes, UpdateButton::Skip]);
        buttons
    }

    pub(super) fn on_update_panel_key(&mut self, code: KeyCode) -> Vec<Command> {
        match code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => {
                self.update_panel = false;
                Vec::new()
            }
            code => UpdateButton::for_key(code)
                .filter(|button| self.update_panel_buttons().contains(button))
                .map_or_default(|button| self.press_update_button(button)),
        }
    }

    /// A click anywhere closes the panel; on a button it also does what the
    /// button says.
    pub(super) fn on_update_panel_click(&mut self, x: u16, y: u16) -> Vec<Command> {
        let point = ratatui::layout::Position { x, y };
        let hit = self
            .update_buttons
            .iter()
            .find(|(rect, _)| rect.contains(point))
            .map(|(_, button)| *button);
        if let Some(button) = hit {
            self.press_update_button(button)
        } else {
            self.update_panel = false;
            Vec::new()
        }
    }

    fn press_update_button(&mut self, button: UpdateButton) -> Vec<Command> {
        match button {
            UpdateButton::Upgrade => match self.install.upgrade_program() {
                Some(program) => {
                    self.update_panel = false;
                    vec![Command::External(program)]
                }
                None => Vec::new(),
            },
            UpdateButton::Copy => match self.install.command() {
                Some(command) => {
                    self.notify(
                        Level::Success,
                        format!("Copied to the clipboard: {command}"),
                    );
                    vec![Command::Clipboard(command.to_owned())]
                }
                None => Vec::new(),
            },
            UpdateButton::Notes => vec![Command::OpenUrl(RELEASES_URL.to_owned())],
            UpdateButton::Skip => {
                self.update_panel = false;
                self.update_dismissed = self.update_available.take();
                Vec::new()
            }
        }
    }

    /// The upgrade ran and succeeded. The running binary is still the old
    /// one, so the notice goes away and the user is told to restart.
    pub(super) fn on_upgraded(&mut self) -> Vec<Command> {
        self.update_panel = false;
        let version = self
            .update_available
            .take()
            .unwrap_or_else(|| "the new version".to_owned());
        self.notify(
            Level::Success,
            format!("Upgraded to {version}. Restart ggt to use it."),
        );
        Vec::new()
    }
}
