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
//! ```

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::cli::IconStyle;

/// Environment variable naming the config file to use instead of the default.
pub const CONFIG_ENV: &str = "TUI_GIT_GRAPH_CONFIG";

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub icons: Option<IconStyle>,
    pub layout: Layout,
}

/// Pane and column widths; None keeps the built-in default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
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
            }
        );
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
