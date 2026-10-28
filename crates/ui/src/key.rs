/// A key press as the popups understand it.
///
/// The app translates terminal events (crossterm) into this, so this crate
/// stays independent of the terminal backend and widgets are easy to drive
/// from tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    Enter,
    Esc,
    Tab,
    /// Shift+Tab.
    BackTab,
    Backspace,
    Delete,
    /// A printable character (also Space, as `' '`).
    Char(char),
}
