//! The config file: `~/.config/tui-git-graph/config.toml` (or under
//! `$XDG_CONFIG_HOME`, or wherever `$TUI_GIT_GRAPH_CONFIG` points).
//!
//! ```toml
//! icons = "auto"          # "auto", "nerd" or "unicode"
//!
//! [layout]                # written back when panes or columns are resized
//! source_control_width = 36
//! graph_column = 12
//! date_column = 16
//! author_column = 14
//! hash_column = 8
//! hidden_columns = ["author"]  # hidden from the header's right-click menu:
//!                              # "date", "author" or "commit"
//!
//! [diff]
//! pager = "delta"         # pipe file diffs into this command; "" or unset:
//!                         # the built-in view ($TUI_GIT_GRAPH_PAGER wins)
//!
//! [issues]                # `#123` in commit messages opens this page;
//! url = "https://jira.example.com/browse/PROJ-$1"   # $1 is the number
//! prefix = "PROJ-"        # what starts a reference; default "#"
//! ```

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tgg_actions::GraphColumn;

use crate::cli::IconStyle;

/// Environment variable naming the config file to use instead of the default.
pub const CONFIG_ENV: &str = "TUI_GIT_GRAPH_CONFIG";

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub icons: Option<IconStyle>,
    pub layout: Layout,
    pub diff: Diff,
    pub issues: Issues,
}

/// Issue links in commit messages (S20). Unset, `#123` opens issue 123 on
/// the origin's GitHub, GitLab or Bitbucket page.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Issues {
    /// The issue page, with `$1` where the number goes.
    pub url: Option<String>,
    /// What starts a reference, such as `PROJ-`; None is `#`.
    pub prefix: Option<String>,
}

impl Issues {
    /// What starts an issue reference.
    pub fn prefix(&self) -> &str {
        self.prefix
            .as_deref()
            .filter(|p| !p.is_empty())
            .unwrap_or("#")
    }

    /// The configured page of issue `number`, if a URL is configured.
    pub fn url_for(&self, number: &str) -> Option<String> {
        self.url
            .as_deref()
            .filter(|url| !url.is_empty())
            .map(|url| url.replace("$1", number))
    }
}

/// How file diffs are shown.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Diff {
    /// Shell command the colored diff is piped into, such as `delta`; None
    /// (or empty) uses the built-in diff view.
    pub pager: Option<String>,
}

/// Pane and column widths and hidden columns; None keeps the built-in default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Layout {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_control_width: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph_column: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_column: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_column: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash_column: Option<u16>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hidden_columns: Vec<ColumnName>,
}

/// A graph column the header's menu can hide, as the config file names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColumnName {
    Date,
    Author,
    Commit,
}

impl From<ColumnName> for GraphColumn {
    fn from(name: ColumnName) -> Self {
        match name {
            ColumnName::Date => Self::Date,
            ColumnName::Author => Self::Author,
            ColumnName::Commit => Self::Commit,
        }
    }
}

impl From<GraphColumn> for ColumnName {
    fn from(column: GraphColumn) -> Self {
        match column {
            GraphColumn::Date => Self::Date,
            GraphColumn::Author => Self::Author,
            GraphColumn::Commit => Self::Commit,
        }
    }
}

/// Where the config file lives; `env` looks up an environment variable.
pub fn path(env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    if let Some(path) = env(CONFIG_ENV) {
        return Some(PathBuf::from(path));
    }
    let base = env("XDG_CONFIG_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            // HOME on Unix, USERPROFILE on Windows.
            env("HOME")
                .or_else(|| env("USERPROFILE"))
                .filter(|dir| !dir.is_empty())
                .map(|home| PathBuf::from(home).join(".config"))
        })?;
    Some(base.join("tui-git-graph").join("config.toml"))
}

/// Reads the config file; a missing file is the default config.
pub fn load(path: &Path) -> Result<Config> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Config::default()),
        Err(err) => return Err(err).with_context(|| format!("cannot read {}", path.display())),
    };
    toml::from_str(&text).with_context(|| format!("invalid config file {}", path.display()))
}

/// Writes `layout` into the config file, keeping its other settings.
pub fn save_layout(path: &Path, layout: Layout) -> Result<()> {
    let mut table: toml::Table = match fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text)
            .with_context(|| format!("invalid config file {}", path.display()))?,
        Err(err) if err.kind() == ErrorKind::NotFound => toml::Table::new(),
        Err(err) => return Err(err).with_context(|| format!("cannot read {}", path.display())),
    };
    let layout = toml::Table::try_from(layout).context("cannot write the layout")?;
    table.insert("layout".into(), toml::Value::Table(layout));
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    // Write a sibling file and rename it over the config, so a crash or a
    // full disk never leaves a half-written config behind.
    let partial = path.with_extension("toml.partial");
    fs::write(&partial, toml::to_string(&table)?)
        .with_context(|| format!("cannot write {}", partial.display()))?;
    fs::rename(&partial, path).with_context(|| format!("cannot write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let vars: Vec<(String, String)> = vars
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |name| vars.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
    }

    #[test]
    fn the_file_lives_under_the_config_directory() {
        assert_eq!(
            path(&env(&[("XDG_CONFIG_HOME", "/xdg")])),
            Some(PathBuf::from("/xdg/tui-git-graph/config.toml"))
        );
        assert_eq!(
            path(&env(&[
                (CONFIG_ENV, "/elsewhere.toml"),
                ("XDG_CONFIG_HOME", "/xdg")
            ])),
            Some(PathBuf::from("/elsewhere.toml"))
        );
        assert_eq!(
            path(&env(&[("HOME", "/home/me")])),
            Some(PathBuf::from("/home/me/.config/tui-git-graph/config.toml"))
        );
        assert_eq!(
            path(&env(&[("USERPROFILE", "C:/Users/me")])),
            Some(PathBuf::from(
                "C:/Users/me/.config/tui-git-graph/config.toml"
            ))
        );
        assert_eq!(path(&env(&[])), None, "no home, no config file");
    }

    #[test]
    fn a_missing_file_is_the_default() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            load(&dir.path().join("none.toml")).unwrap(),
            Config::default()
        );
    }

    #[test]
    fn reads_icons_and_layout() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        fs::write(
            &file,
            "icons = \"nerd\"\n[layout]\nsource_control_width = 40\ndate_column = 20\n",
        )
        .unwrap();
        assert_eq!(
            load(&file).unwrap(),
            Config {
                icons: Some(IconStyle::Nerd),
                layout: Layout {
                    source_control_width: Some(40),
                    date_column: Some(20),
                    ..Layout::default()
                },
                diff: Diff::default(),
                issues: Issues::default(),
            }
        );
    }

    #[test]
    fn reads_the_diff_pager() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        fs::write(&file, "[diff]\npager = \"delta --side-by-side\"\n").unwrap();
        assert_eq!(
            load(&file).unwrap().diff.pager.as_deref(),
            Some("delta --side-by-side")
        );
        fs::write(&file, "[diff]\nviewer = \"delta\"\n").unwrap();
        let err = format!("{:#}", load(&file).unwrap_err());
        assert!(err.contains("unknown field `viewer`"), "{err}");
    }

    #[test]
    fn reads_the_issue_links() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        fs::write(
            &file,
            "[issues]\nurl = \"https://jira.example.com/browse/PROJ-$1\"\nprefix = \"PROJ-\"\n",
        )
        .unwrap();
        let issues = load(&file).unwrap().issues;
        assert_eq!(issues.prefix(), "PROJ-");
        assert_eq!(
            issues.url_for("42").as_deref(),
            Some("https://jira.example.com/browse/PROJ-42")
        );
        let unset = Issues::default();
        assert_eq!((unset.prefix(), unset.url_for("1")), ("#", None));
        fs::write(&file, "[issues]\nlink = \"x\"\n").unwrap();
        let err = format!("{:#}", load(&file).unwrap_err());
        assert!(err.contains("unknown field `link`"), "{err}");
    }

    #[test]
    fn saving_the_layout_keeps_the_issue_links() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        fs::write(&file, "[issues]\nprefix = \"GH-\"\n").unwrap();
        save_layout(&file, Layout::default()).unwrap();
        assert_eq!(load(&file).unwrap().issues.prefix(), "GH-");
    }

    #[test]
    fn saving_the_layout_keeps_the_pager() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        fs::write(&file, "[diff]\npager = \"delta\"\n").unwrap();
        save_layout(&file, Layout::default()).unwrap();
        assert_eq!(load(&file).unwrap().diff.pager.as_deref(), Some("delta"));
    }

    #[test]
    fn hidden_columns_are_saved_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        save_layout(
            &file,
            Layout {
                hidden_columns: vec![ColumnName::Author, ColumnName::Commit],
                ..Layout::default()
            },
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "[layout]\nhidden_columns = [\"author\", \"commit\"]\n"
        );
        assert_eq!(
            load(&file).unwrap().layout.hidden_columns,
            [ColumnName::Author, ColumnName::Commit]
        );
        fs::write(&file, "[layout]\nhidden_columns = [\"graph\"]\n").unwrap();
        let err = format!("{:#}", load(&file).unwrap_err());
        assert!(err.contains("unknown variant `graph`"), "{err}");
    }

    #[test]
    fn mistakes_name_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        fs::write(&file, "icons = \"emoji\"\n").unwrap();
        let err = format!("{:#}", load(&file).unwrap_err());
        assert!(
            err.starts_with(&format!("invalid config file {}", file.display())),
            "{err}"
        );
        fs::write(&file, "colour = 1\n").unwrap();
        let err = format!("{:#}", load(&file).unwrap_err());
        assert!(err.contains("unknown field `colour`"), "{err}");
    }

    #[test]
    fn saving_the_layout_keeps_other_settings() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("sub").join("config.toml");
        save_layout(
            &file,
            Layout {
                source_control_width: Some(33),
                ..Layout::default()
            },
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "[layout]\nsource_control_width = 33\n"
        );
        fs::write(&file, "icons = \"unicode\"\n[layout]\nhash_column = 9\n").unwrap();
        save_layout(
            &file,
            Layout {
                graph_column: Some(10),
                ..Layout::default()
            },
        )
        .unwrap();
        let config = load(&file).unwrap();
        assert_eq!(config.icons, Some(IconStyle::Unicode));
        assert_eq!(
            config.layout,
            Layout {
                graph_column: Some(10),
                ..Layout::default()
            }
        );
    }
}
