//! Dark or light colors, picked to match the terminal's background.

use ggt_ui::Theme;
use serde::Deserialize;

/// The `theme` setting in the config file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice {
    /// Ask the terminal for its background color.
    #[default]
    Auto,
    Dark,
    Light,
}

/// Whether the terminal's background is dark or light.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Background {
    Dark,
    Light,
}

impl Background {
    pub fn theme(self) -> Theme {
        match self {
            Self::Dark => Theme::dark(),
            Self::Light => Theme::light(),
        }
    }
}

/// The background to draw for. A `dark` or `light` choice wins; `auto` asks
/// the terminal (`query`), then reads `$COLORFGBG`, and falls back to dark,
/// the look the app always had.
pub fn background(
    choice: ThemeChoice,
    query: &dyn Fn() -> Option<Background>,
    env: &dyn Fn(&str) -> Option<String>,
) -> Background {
    match choice {
        ThemeChoice::Dark => Background::Dark,
        ThemeChoice::Light => Background::Light,
        ThemeChoice::Auto => query()
            .or_else(|| env("COLORFGBG").as_deref().and_then(from_colorfgbg))
            .unwrap_or(Background::Dark),
    }
}

/// Asks the terminal for its colors (an OSC 11 query). `None` when it
/// doesn't answer, as in a pipe or a terminal without support. Runs before
/// the app takes over the terminal, so the answer isn't read as a key press.
pub fn query_terminal() -> Option<Background> {
    match terminal_colorsaurus::theme_mode(terminal_colorsaurus::QueryOptions::default()) {
        Ok(terminal_colorsaurus::ThemeMode::Dark) => Some(Background::Dark),
        Ok(terminal_colorsaurus::ThemeMode::Light) => Some(Background::Light),
        Err(_) => None,
    }
}

/// `$COLORFGBG` (set by rxvt, Konsole and others) is `fg;bg` or
/// `fg;default;bg`, with ANSI color numbers. Backgrounds 7 (white) and 9..=15
/// (bright colors) are light; 0..=6 and 8 are dark.
fn from_colorfgbg(value: &str) -> Option<Background> {
    let bg: u8 = value.rsplit(';').next()?.trim().parse().ok()?;
    match bg {
        0..=6 | 8 => Some(Background::Dark),
        7 | 9..=15 => Some(Background::Light),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{Background, ThemeChoice, background};

    fn env_with(colorfgbg: Option<&str>) -> impl Fn(&str) -> Option<String> {
        let value = colorfgbg.map(str::to_owned);
        move |name| (name == "COLORFGBG").then(|| value.clone()).flatten()
    }

    #[test]
    fn auto_follows_the_terminals_answer() {
        let none = env_with(None);
        assert_eq!(
            background(ThemeChoice::Auto, &|| Some(Background::Light), &none),
            Background::Light
        );
        assert_eq!(
            background(ThemeChoice::Auto, &|| Some(Background::Dark), &none),
            Background::Dark
        );
    }

    #[test]
    fn a_set_theme_wins_without_asking_the_terminal() {
        let none = env_with(None);
        let never = || -> Option<Background> { panic!("the terminal was queried") };
        assert_eq!(
            background(ThemeChoice::Light, &never, &none),
            Background::Light
        );
        assert_eq!(
            background(ThemeChoice::Dark, &never, &none),
            Background::Dark
        );
    }

    #[test]
    fn without_an_answer_colorfgbg_decides() {
        let silent = || None;
        for (value, expected) in [
            ("0;15", Background::Light),
            ("0;7", Background::Light),
            ("15;0", Background::Dark),
            ("15;default;0", Background::Dark),
            ("0;default;15", Background::Light),
        ] {
            let env = env_with(Some(value));
            assert_eq!(
                background(ThemeChoice::Auto, &silent, &env),
                expected,
                "{value}"
            );
        }
    }

    #[test]
    fn without_any_hint_it_stays_dark() {
        let silent = || None;
        for colorfgbg in [None, Some("garbage"), Some("0;default")] {
            let env = env_with(colorfgbg);
            assert_eq!(
                background(ThemeChoice::Auto, &silent, &env),
                Background::Dark,
                "{colorfgbg:?}"
            );
        }
    }
}
