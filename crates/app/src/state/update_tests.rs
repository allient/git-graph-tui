//! The "Update available" panel: upgrade in place, copy the command, release
//! notes and skipping a version, by keyboard and by mouse.

use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ggt_testkit::FixtureRepo;
use pretty_assertions::assert_eq;

use super::*;
use crate::testing::Harness;
use crate::upgrade::{BREW_UPGRADE, Install, POWERSHELL_INSTALL, SHELL_INSTALL};

const W: u16 = 110;
const H: u16 = 24;

fn with_update(install: Install) -> (FixtureRepo, Harness) {
    let repo = FixtureRepo::sample();
    let mut h = Harness::open(&repo);
    h.state.install = install;
    h.state.update_available = Some("v9.0.0".into());
    h.key(KeyCode::Char('U'));
    h.draw(W, H);
    assert!(h.state.update_panel);
    (repo, h)
}

fn click_button(h: &mut Harness, button: UpdateButton) {
    let rect = h
        .state
        .update_buttons
        .iter()
        .find(|(_, b)| *b == button)
        .map_or_else(|| panic!("{button:?} is not on screen"), |(r, _)| *r);
    let commands = h.state.update(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: rect.x + 1,
        row: rect.y,
        modifiers: KeyModifiers::NONE,
    }));
    h.settle(commands);
    h.draw(W, H);
}

fn buttons(h: &Harness) -> Vec<UpdateButton> {
    h.state.update_buttons.iter().map(|(_, b)| *b).collect()
}

#[test]
fn homebrew_shows_its_command_and_every_button() {
    let (_repo, h) = with_update(Install::Homebrew);
    assert!(h.find_on_screen(BREW_UPGRADE).is_some());
    assert!(h.find_on_screen("Upgrade now").is_some());
    assert_eq!(
        buttons(&h),
        [
            UpdateButton::Upgrade,
            UpdateButton::Copy,
            UpdateButton::Notes,
            UpdateButton::Skip
        ]
    );
}

#[test]
fn u_upgrades_in_the_terminal_and_closes_the_panel() {
    let (_repo, mut h) = with_update(Install::Homebrew);
    h.key(KeyCode::Char('u'));
    assert!(!h.state.update_panel);
    let [program] = h.external.as_slice() else {
        panic!("expected one program, got {:?}", h.external);
    };
    assert!(program.upgrade);
    assert!(
        program.args.iter().any(|a| a == BREW_UPGRADE),
        "{program:?}"
    );
}

#[test]
fn the_upgrade_button_runs_the_same_command() {
    let (_repo, mut h) = with_update(Install::ShellInstaller);
    click_button(&mut h, UpdateButton::Upgrade);
    assert_eq!(h.external.len(), 1);
    assert!(h.external[0].args.iter().any(|a| a == SHELL_INSTALL));
}

#[test]
fn c_copies_the_command_and_says_so() {
    let (_repo, mut h) = with_update(Install::Homebrew);
    h.key(KeyCode::Char('c'));
    assert_eq!(h.clipboard, [BREW_UPGRADE]);
    assert_eq!(h.state.message_level, Level::Success);
    assert!(
        h.state
            .message
            .as_deref()
            .unwrap_or("")
            .contains(BREW_UPGRADE)
    );
    assert_eq!(h.external, Vec::new());
}

#[test]
fn the_copy_button_copies_too() {
    let (_repo, mut h) = with_update(Install::ShellInstaller);
    click_button(&mut h, UpdateButton::Copy);
    assert_eq!(h.clipboard, [SHELL_INSTALL]);
}

#[test]
fn a_finished_upgrade_clears_the_notice_and_asks_for_a_restart() {
    let (_repo, mut h) = with_update(Install::Homebrew);
    h.key(KeyCode::Char('u'));
    let commands = h.state.update(Event::Upgraded);
    h.settle(commands);
    assert!(h.state.update_available.is_none());
    assert!(!h.state.update_panel);
    assert_eq!(h.state.message_level, Level::Success);
    let message = h.state.message.clone().unwrap_or_default();
    assert!(message.contains("v9.0.0"), "{message}");
    assert!(message.contains("Restart"), "{message}");
}

#[test]
fn windows_installs_copy_the_command_but_never_run_it() {
    // ggt.exe can't be replaced while it runs, so the installer needs the
    // app closed first.
    let (_repo, mut h) = with_update(Install::PowerShellInstaller);
    assert!(h.find_on_screen("Quit ggt").is_some());
    assert_eq!(
        buttons(&h),
        [UpdateButton::Copy, UpdateButton::Notes, UpdateButton::Skip]
    );
    h.key(KeyCode::Char('u'));
    assert_eq!(h.external, Vec::new());
    assert!(h.state.update_panel);
    h.key(KeyCode::Char('c'));
    assert_eq!(h.clipboard, [POWERSHELL_INSTALL]);
}

#[test]
fn an_unknown_install_points_to_the_release_page() {
    let (_repo, mut h) = with_update(Install::Unknown);
    assert_eq!(buttons(&h), [UpdateButton::Notes, UpdateButton::Skip]);
    h.key(KeyCode::Char('c'));
    h.key(KeyCode::Char('u'));
    assert_eq!(h.clipboard, Vec::<String>::new());
    assert_eq!(h.external, Vec::new());
    click_button(&mut h, UpdateButton::Notes);
    assert_eq!(
        h.opened,
        ["https://github.com/allient/git-graph-tui/releases/latest"]
    );
}

#[test]
fn skip_dismisses_this_version_by_key_and_by_button() {
    let (_repo, mut h) = with_update(Install::Homebrew);
    h.key(KeyCode::Char('d'));
    assert_eq!(h.state.update_dismissed.as_deref(), Some("v9.0.0"));
    let (_repo, mut h) = with_update(Install::Homebrew);
    click_button(&mut h, UpdateButton::Skip);
    assert_eq!(h.state.update_dismissed.as_deref(), Some("v9.0.0"));
    assert!(!h.state.update_panel);
}

#[test]
fn a_click_outside_the_buttons_closes_the_panel() {
    let (_repo, mut h) = with_update(Install::Homebrew);
    let commands = h.state.update(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 0,
        row: 0,
        modifiers: KeyModifiers::NONE,
    }));
    h.settle(commands);
    assert!(!h.state.update_panel);
    assert_eq!(h.external, Vec::new());
    assert_eq!(h.clipboard, Vec::<String>::new());
}
