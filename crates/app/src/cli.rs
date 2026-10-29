//! Command-line interface.

use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::{Parser, ValueEnum};
use tgg_ui::Icons;

/// Environment variable that picks the icons when `--icons` isn't given.
pub const ICONS_ENV: &str = "TUI_GIT_GRAPH_ICONS";

/// A Git Graph for the terminal.
#[derive(Debug, Parser)]
#[command(name = "tui-git-graph", version, about, long_about = None)]
pub struct Cli {
    /// Path inside the repository to open.
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Toolbar icons: plain Unicode, or VS Code's icons from a Nerd Font
    /// (install one first). Defaults to `$TUI_GIT_GRAPH_ICONS`, then unicode.
    #[arg(long, value_enum, value_name = "STYLE")]
    pub icons: Option<IconStyle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum IconStyle {
    Unicode,
    Nerd,
}

impl From<IconStyle> for Icons {
    fn from(style: IconStyle) -> Self {
        match style {
            IconStyle::Unicode => Self::Unicode,
            IconStyle::Nerd => Self::Nerd,
        }
    }
}

impl Cli {
    /// The icons to draw: the flag, else `env` (the value of [`ICONS_ENV`]),
    /// else plain Unicode.
    pub fn icons(&self, env: Option<&str>) -> Result<Icons> {
        if let Some(style) = self.icons {
            return Ok(style.into());
        }
        let Some(value) = env else {
            return Ok(Icons::Unicode);
        };
        match IconStyle::from_str(value, true) {
            Ok(style) => Ok(style.into()),
            Err(_) => bail!("{ICONS_ENV} must be \"unicode\" or \"nerd\", not \"{value}\""),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;
    use tgg_ui::Icons;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn path_defaults_to_current_directory() {
        let cli = Cli::try_parse_from(["tui-git-graph"]).unwrap();
        assert_eq!(cli.path, PathBuf::from("."));
    }

    #[test]
    fn icons_are_plain_unless_asked_for() {
        let cli = Cli::try_parse_from(["tui-git-graph"]).unwrap();
        assert_eq!(cli.icons(None).unwrap(), Icons::Unicode);
    }

    #[test]
    fn the_icons_flag_picks_nerd_font_icons() {
        let cli = Cli::try_parse_from(["tui-git-graph", "--icons", "nerd"]).unwrap();
        assert_eq!(cli.icons(None).unwrap(), Icons::Nerd);
    }

    #[test]
    fn the_environment_variable_picks_icons_when_the_flag_is_missing() {
        let cli = Cli::try_parse_from(["tui-git-graph"]).unwrap();
        assert_eq!(cli.icons(Some("nerd")).unwrap(), Icons::Nerd);
        assert_eq!(cli.icons(Some("Unicode")).unwrap(), Icons::Unicode);
    }

    #[test]
    fn the_flag_wins_over_the_environment_variable() {
        let cli = Cli::try_parse_from(["tui-git-graph", "--icons", "unicode"]).unwrap();
        assert_eq!(cli.icons(Some("nerd")).unwrap(), Icons::Unicode);
    }

    #[test]
    fn an_unknown_environment_value_is_an_error() {
        let cli = Cli::try_parse_from(["tui-git-graph"]).unwrap();
        let err = cli.icons(Some("emoji")).unwrap_err();
        assert_eq!(
            err.to_string(),
            "TUI_GIT_GRAPH_ICONS must be \"unicode\" or \"nerd\", not \"emoji\""
        );
    }

    #[test]
    fn an_unknown_flag_value_is_rejected() {
        assert!(Cli::try_parse_from(["tui-git-graph", "--icons", "emoji"]).is_err());
    }

    #[test]
    fn path_can_be_given_as_positional_argument() {
        let cli = Cli::try_parse_from(["tui-git-graph", "../other-repo"]).unwrap();
        assert_eq!(cli.path, PathBuf::from("../other-repo"));
    }
}
