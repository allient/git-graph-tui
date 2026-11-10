//! Looks for an installed Nerd Font, so `--icons auto` can pick the icons
//! in terminals that don't bundle the symbols (Terminal.app, iTerm2, ...).

use std::fs;
use std::path::{Path, PathBuf};

/// How deep to look inside a font folder. Linux packages nest fonts one or two
/// folders down (`fonts/truetype/<family>/`); macOS keeps them flat.
const MAX_DEPTH: usize = 3;
/// Stops a huge system font folder from slowing down startup.
const MAX_ENTRIES: usize = 20_000;

/// The folders fonts are installed in. `home` is the user's home folder.
fn font_dirs(home: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/Library/Fonts"),
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
    ];
    if let Some(home) = home {
        dirs.push(home.join("Library/Fonts"));
        dirs.push(home.join(".local/share/fonts"));
        dirs.push(home.join(".fonts"));
    }
    dirs
}

/// Whether a font file or family folder is a Nerd Font. The patched fonts are
/// named `JetBrainsMonoNerdFont-Regular.ttf`, `Hack Nerd Font Mono.ttf`,
/// `FiraCodeNFM-Reg.ttf`, ...
fn is_nerd_font_name(name: &str) -> bool {
    let name = name.to_lowercase();
    let is_font = [".ttf", ".otf", ".ttc"]
        .iter()
        .any(|ext| name.ends_with(ext));
    is_font
        && (name.contains("nerdfont") || name.contains("nerd font") || name.contains("nerd-font"))
}

fn has_nerd_font(dir: &Path, depth: usize, seen: &mut usize) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        *seen += 1;
        if *seen > MAX_ENTRIES {
            return false;
        }
        let path = entry.path();
        if path.is_dir() {
            if depth < MAX_DEPTH && has_nerd_font(&path, depth + 1, seen) {
                return true;
            }
        } else if entry.file_name().to_str().is_some_and(is_nerd_font_name) {
            return true;
        }
    }
    false
}

/// Whether any Nerd Font is installed. Installed isn't the same as selected in
/// the terminal, so this is a hint: `--icons unicode` is the way out.
pub fn nerd_font_installed(env: &dyn Fn(&str) -> Option<String>) -> bool {
    let home = env("HOME").map(PathBuf::from);
    nerd_font_in(&font_dirs(home.as_deref()))
}

fn nerd_font_in(dirs: &[PathBuf]) -> bool {
    let mut seen = 0;
    dirs.iter().any(|dir| has_nerd_font(dir, 0, &mut seen))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_nerd_font_files() {
        for name in [
            "JetBrainsMonoNerdFont-Regular.ttf",
            "Hack Nerd Font Mono Bold.ttf",
            "FiraCode-NerdFont.otf",
            "MesloLGS NERD FONT.TTC",
        ] {
            assert!(is_nerd_font_name(name), "{name}");
        }
        for name in ["Menlo.ttc", "Nerdy.ttf", "NerdFont-readme.txt", "SFNS.ttf"] {
            assert!(!is_nerd_font_name(name), "{name}");
        }
    }

    #[test]
    fn finds_a_nerd_font_in_a_nested_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let family = tmp.path().join("truetype/jetbrains");
        fs::create_dir_all(&family).unwrap();
        assert!(!nerd_font_in(&[tmp.path().to_path_buf()]));
        fs::write(family.join("JetBrainsMonoNerdFont-Regular.ttf"), b"").unwrap();
        assert!(nerd_font_in(&[tmp.path().to_path_buf()]));
    }

    #[test]
    fn a_missing_folder_has_no_font() {
        assert!(!nerd_font_in(&[PathBuf::from("/definitely/not/here")]));
    }
}
