//! Command-line interface.

use std::path::PathBuf;

use clap::Parser;

/// A Git Graph for the terminal.
#[derive(Debug, Parser)]
#[command(name = "tui-git-graph", version, about, long_about = None)]
pub struct Cli {
    /// Path inside the repository to open.
    #[arg(default_value = ".")]
    pub path: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

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
    fn path_can_be_given_as_positional_argument() {
        let cli = Cli::try_parse_from(["tui-git-graph", "../other-repo"]).unwrap();
        assert_eq!(cli.path, PathBuf::from("../other-repo"));
    }
}
