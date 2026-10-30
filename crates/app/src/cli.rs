//! Command-line interface.

use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::{Parser, ValueEnum};
use ggt_ui::Icons;

/// Environment variable that picks the icons when `--icons` isn't given.
pub const ICONS_ENV: &str = "GIT_GRAPH_TUI_ICONS";

/// A Git Graph for the terminal.
#[derive(Debug, Parser)]
#[command(name = "git-graph-tui", version, about, long_about = None)]
pub struct Cli {
    /// Path inside the repository to open.
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Toolbar icons: VS Code's icons from a Nerd Font, or plain Unicode.
    /// `auto` uses the icons in terminals that ship Nerd Font symbols
    /// (Ghostty, WezTerm, Kitty). Defaults to `$GIT_GRAPH_TUI_ICONS`, then auto.
    #[arg(long, value_enum, value_name = "STYLE")]
    pub icons: Option<IconStyle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IconStyle {
    Auto,
    Unicode,
    Nerd,
}

impl Cli {
    /// The icons to draw: the flag, else [`ICONS_ENV`], else the config
    /// file's `icons`, else `auto`, which picks Nerd Font icons when the
    /// terminal is known to have them. `env` looks up an environment variable.
    pub fn icons(
        &self,
        env: &dyn Fn(&str) -> Option<String>,
        config: Option<IconStyle>,
    ) -> Result<Icons> {
        let style = match (self.icons, env(ICONS_ENV)) {
            (Some(style), _) => style,
            (None, None) => config.unwrap_or(IconStyle::Auto),
            (None, Some(value)) => match IconStyle::from_str(&value, true) {
                Ok(style) => style,
                Err(_) => {
                    bail!("{ICONS_ENV} must be \"auto\", \"unicode\" or \"nerd\", not \"{value}\"")
                }
            },
        };
        Ok(match style {
            IconStyle::Nerd => Icons::Nerd,
            IconStyle::Auto if ships_nerd_font_symbols(env) => Icons::Nerd,
            IconStyle::Auto | IconStyle::Unicode => Icons::Unicode,
        })
    }
}

/// Whether the terminal draws Nerd Font symbols whatever font the user picked:
/// Ghostty, WezTerm and Kitty bundle them. Their variables survive inside
/// tmux or herdr, where the outer terminal still draws the cells.
fn ships_nerd_font_symbols(env: &dyn Fn(&str) -> Option<String>) -> bool {
    let program = env("TERM_PROGRAM").unwrap_or_default().to_lowercase();
    let term = env("TERM").unwrap_or_default();
    matches!(program.as_str(), "ghostty" | "wezterm")
        || matches!(term.as_str(), "xterm-ghostty" | "xterm-kitty")
        || [
            "GHOSTTY_RESOURCES_DIR",
            "WEZTERM_EXECUTABLE",
            "KITTY_WINDOW_ID",
        ]
        .iter()
        .any(|name| env(name).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use ggt_ui::Icons;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn path_defaults_to_current_directory() {
        let cli = Cli::try_parse_from(["git-graph-tui"]).unwrap();
        assert_eq!(cli.path, PathBuf::from("."));
    }

    /// An environment with only `vars` set.
    fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let vars: Vec<(String, String)> = vars
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |name| vars.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
    }

    fn icons(args: &[&str], vars: &[(&str, &str)]) -> Result<Icons> {
        let cli = Cli::try_parse_from(std::iter::once("git-graph-tui").chain(args.iter().copied()))
            .unwrap();
        cli.icons(&env(vars), None)
    }

    #[test]
    fn icons_are_plain_in_an_unknown_terminal() {
        assert_eq!(icons(&[], &[]).unwrap(), Icons::Unicode);
        assert_eq!(
            icons(&[], &[("TERM_PROGRAM", "Apple_Terminal")]).unwrap(),
            Icons::Unicode
        );
    }

    #[test]
    fn terminals_that_ship_nerd_font_symbols_get_icons_automatically() {
        for vars in [
            [("TERM_PROGRAM", "ghostty")],
            [(
                "GHOSTTY_RESOURCES_DIR",
                "/Applications/Ghostty.app/Contents/Resources/ghostty",
            )],
            [("TERM_PROGRAM", "WezTerm")],
            [("TERM", "xterm-kitty")],
            [("KITTY_WINDOW_ID", "1")],
        ] {
            assert_eq!(icons(&[], &vars).unwrap(), Icons::Nerd, "{vars:?}");
        }
        // Inside a multiplexer the outer terminal still draws the icons.
        assert_eq!(
            icons(
                &[],
                &[
                    ("TERM_PROGRAM", "herdr"),
                    ("GHOSTTY_RESOURCES_DIR", "/Applications/Ghostty.app")
                ]
            )
            .unwrap(),
            Icons::Nerd
        );
    }

    #[test]
    fn the_icons_flag_forces_either_style() {
        assert_eq!(icons(&["--icons", "nerd"], &[]).unwrap(), Icons::Nerd);
        assert_eq!(
            icons(&["--icons", "unicode"], &[("TERM_PROGRAM", "ghostty")]).unwrap(),
            Icons::Unicode
        );
        assert_eq!(icons(&["--icons", "auto"], &[]).unwrap(), Icons::Unicode);
    }

    #[test]
    fn the_environment_variable_picks_icons_when_the_flag_is_missing() {
        assert_eq!(
            icons(&[], &[("GIT_GRAPH_TUI_ICONS", "nerd")]).unwrap(),
            Icons::Nerd
        );
        assert_eq!(
            icons(
                &[],
                &[
                    ("GIT_GRAPH_TUI_ICONS", "Unicode"),
                    ("TERM_PROGRAM", "ghostty")
                ]
            )
            .unwrap(),
            Icons::Unicode
        );
        assert_eq!(
            icons(
                &[],
                &[("GIT_GRAPH_TUI_ICONS", "auto"), ("TERM_PROGRAM", "ghostty")]
            )
            .unwrap(),
            Icons::Nerd
        );
    }

    #[test]
    fn the_flag_wins_over_the_environment_variable() {
        assert_eq!(
            icons(&["--icons", "unicode"], &[("GIT_GRAPH_TUI_ICONS", "nerd")]).unwrap(),
            Icons::Unicode
        );
    }

    #[test]
    fn an_unknown_environment_value_is_an_error() {
        let err = icons(&[], &[("GIT_GRAPH_TUI_ICONS", "emoji")]).unwrap_err();
        assert_eq!(
            err.to_string(),
            "GIT_GRAPH_TUI_ICONS must be \"auto\", \"unicode\" or \"nerd\", not \"emoji\""
        );
    }

    #[test]
    fn an_unknown_flag_value_is_rejected() {
        assert!(Cli::try_parse_from(["git-graph-tui", "--icons", "emoji"]).is_err());
    }

    #[test]
    fn path_can_be_given_as_positional_argument() {
        let cli = Cli::try_parse_from(["git-graph-tui", "../other-repo"]).unwrap();
        assert_eq!(cli.path, PathBuf::from("../other-repo"));
    }

    #[test]
    fn the_config_file_picks_icons_after_the_flag_and_the_variable() {
        let cli = Cli::try_parse_from(["git-graph-tui"]).unwrap();
        assert_eq!(
            cli.icons(&env(&[]), Some(IconStyle::Nerd)).unwrap(),
            Icons::Nerd
        );
        assert_eq!(
            cli.icons(
                &env(&[("GIT_GRAPH_TUI_ICONS", "unicode")]),
                Some(IconStyle::Nerd)
            )
            .unwrap(),
            Icons::Unicode
        );
        let flag = Cli::try_parse_from(["git-graph-tui", "--icons", "unicode"]).unwrap();
        assert_eq!(
            flag.icons(&env(&[]), Some(IconStyle::Nerd)).unwrap(),
            Icons::Unicode
        );
    }
}
