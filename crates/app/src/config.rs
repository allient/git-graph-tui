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
///
/// Only the text of the `[layout]` table is replaced (or appended when the
/// file has none): comments, blank lines, key order and every other table
/// stay byte for byte. Comments inside `[layout]` itself are lost. A file
/// that sets the layout another way (`layout.x = 1` dotted keys, an inline
/// `layout = { ... }`) is rewritten whole, which drops its comments.
pub fn save_layout(path: &Path, layout: Layout) -> Result<()> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err).with_context(|| format!("cannot read {}", path.display())),
    };
    let mut table: toml::Table =
        toml::from_str(&text).with_context(|| format!("invalid config file {}", path.display()))?;
    let layout = toml::Table::try_from(layout).context("cannot write the layout")?;
    let mut section = toml::Table::new();
    section.insert("layout".into(), toml::Value::Table(layout.clone()));
    let section = toml::to_string(&section).context("cannot write the layout")?;
    table.insert("layout".into(), toml::Value::Table(layout));
    let spliced = splice_layout(&text, &section);
    // The splice only understands a plain `[layout]` header; when its result
    // does not read back as the intended settings, rewrite the whole file.
    let contents = if toml::from_str::<toml::Table>(&spliced).ok().as_ref() == Some(&table) {
        spliced
    } else {
        toml::to_string(&table)?
    };
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    // Write a sibling file and rename it over the config, so a crash or a
    // full disk never leaves a half-written config behind.
    let partial = path.with_extension("toml.partial");
    fs::write(&partial, contents).with_context(|| format!("cannot write {}", partial.display()))?;
    fs::rename(&partial, path).with_context(|| format!("cannot write {}", path.display()))
}

/// `text` with its `[layout]` table replaced by `section`, or `section`
/// appended. The table runs from its header to the next table header; the
/// comments and blank lines right before that header belong to the next
/// table and are kept.
fn splice_layout(text: &str, section: &str) -> String {
    let mut lines = Vec::new();
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        lines.push((offset, line));
        offset += line.len();
    }
    let Some(header) = lines
        .iter()
        .position(|(_, line)| table_header(line) == Some("layout"))
    else {
        let mut out = text.to_owned();
        if !out.is_empty() {
            if !out.ends_with('\n') {
                out.push('\n');
            }
            out.push('\n');
        }
        out.push_str(section);
        return out;
    };
    let next = lines[header + 1..]
        .iter()
        .position(|(_, line)| table_header(line).is_some())
        .map_or(lines.len(), |i| header + 1 + i);
    let mut end = next;
    if next < lines.len() {
        while end > header + 1 && {
            let line = lines[end - 1].1.trim();
            line.is_empty() || line.starts_with('#')
        } {
            end -= 1;
        }
    }
    let start = lines[header].0;
    let end = lines.get(end).map_or(text.len(), |(offset, _)| *offset);
    format!("{}{section}{}", &text[..start], &text[end..])
}

/// The name in a `[name]` or `[[name]]` header line, if `line` is one.
fn table_header(line: &str) -> Option<&str> {
    let line = line.trim();
    let (open, close) = if line.starts_with("[[") {
        ("[[", "]]")
    } else {
        ("[", "]")
    };
    let rest = line.strip_prefix(open)?;
    let (name, after) = rest.split_once(close)?;
    let after = after.trim_start();
    (after.is_empty() || after.starts_with('#')).then_some(name.trim())
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

    fn width(source_control_width: u16) -> Layout {
        Layout {
            source_control_width: Some(source_control_width),
            ..Layout::default()
        }
    }

    #[test]
    fn saving_the_layout_keeps_comments_and_tables_around_it() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        let before = "# my settings\nicons = \"nerd\"   # glyphs\n\n[issues]\n# Jira\nprefix = \"PROJ-\"\n\n";
        let after = "\n# the pager\n[diff]\npager = \"delta\"  # side by side later\n";
        fs::write(
            &file,
            format!("{before}[layout]\nhash_column = 9\ndate_column = 3\n{after}"),
        )
        .unwrap();
        save_layout(&file, width(40)).unwrap();
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            format!("{before}[layout]\nsource_control_width = 40\n{after}")
        );
    }

    #[test]
    fn saving_the_layout_appends_a_missing_layout_table() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        fs::write(&file, "# mine\n[diff]\npager = \"less\" # paging").unwrap();
        save_layout(&file, width(30)).unwrap();
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "# mine\n[diff]\npager = \"less\" # paging\n\n[layout]\nsource_control_width = 30\n"
        );
    }

    #[test]
    fn saving_the_layout_replaces_it_as_the_last_table() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        fs::write(
            &file,
            "icons = \"auto\" # pick\n\n[layout] # mine\nhidden_columns = [\n  \"author\",\n]\n",
        )
        .unwrap();
        save_layout(&file, width(31)).unwrap();
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "icons = \"auto\" # pick\n\n[layout]\nsource_control_width = 31\n"
        );
    }

    #[test]
    fn a_dotted_layout_key_is_rewritten_with_the_whole_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        fs::write(
            &file,
            "# gone\nlayout.hash_column = 9\n[diff]\npager = \"less\"\n",
        )
        .unwrap();
        save_layout(&file, width(32)).unwrap();
        let config = load(&file).unwrap();
        assert_eq!(config.layout, width(32));
        assert_eq!(config.diff.pager.as_deref(), Some("less"));
    }

    #[test]
    fn saving_the_layout_refuses_an_invalid_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        fs::write(&file, "icons = \n").unwrap();
        let err = format!("{:#}", save_layout(&file, width(32)).unwrap_err());
        assert!(
            err.starts_with(&format!("invalid config file {}", file.display())),
            "{err}"
        );
        assert_eq!(fs::read_to_string(&file).unwrap(), "icons = \n");
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
