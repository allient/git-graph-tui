//! How this copy of `ggt` was installed, and the command that upgrades it.

use std::path::{Path, PathBuf};

use crate::external::External;

pub const BREW_UPGRADE: &str = "brew upgrade allient/tap/git-graph-tui";
pub const SHELL_INSTALL: &str = "curl --proto '=https' --tlsv1.2 -LsSf https://github.com/allient/git-graph-tui/releases/latest/download/git-graph-tui-installer.sh | sh";
pub const POWERSHELL_INSTALL: &str = "powershell -ExecutionPolicy Bypass -c \"irm https://github.com/allient/git-graph-tui/releases/latest/download/git-graph-tui-installer.ps1 | iex\"";

/// Where the running binary came from, as far as its path tells.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Install {
    /// `brew install allient/tap/git-graph-tui`.
    Homebrew,
    /// The release's shell installer, into `$CARGO_HOME/bin`.
    ShellInstaller,
    /// The release's PowerShell installer, into `%CARGO_HOME%\bin`.
    PowerShellInstaller,
    /// Built from source or unpacked by hand.
    #[default]
    Unknown,
}

impl Install {
    /// The command that upgrades this install; None when there is none to
    /// suggest.
    pub fn command(self) -> Option<&'static str> {
        match self {
            Self::Homebrew => Some(BREW_UPGRADE),
            Self::ShellInstaller => Some(SHELL_INSTALL),
            Self::PowerShellInstaller => Some(POWERSHELL_INSTALL),
            Self::Unknown => None,
        }
    }

    /// Whether the app can run the upgrade itself. Windows can't replace
    /// `ggt.exe` while it runs, so there the user quits first.
    pub fn upgrades_in_place(self) -> bool {
        matches!(self, Self::Homebrew | Self::ShellInstaller)
    }

    /// One line on how the app was installed and what to do.
    pub fn summary(self) -> &'static str {
        match self {
            Self::Homebrew => "Installed with Homebrew. Upgrade with:",
            Self::ShellInstaller => {
                "Installed with the install script. Upgrade by running it again:"
            }
            Self::PowerShellInstaller => {
                "Installed with the PowerShell installer. Quit ggt, then run:"
            }
            Self::Unknown => "Download the new version from the release page.",
        }
    }

    /// Runs the upgrade with the terminal handed over, then waits for Enter
    /// so its output stays readable before the app comes back. None when this
    /// install can't upgrade in place.
    pub fn upgrade_program(self) -> Option<External> {
        if !self.upgrades_in_place() {
            return None;
        }
        let command = self.command()?;
        // The command is a constant, passed as `$1` so it is never re-quoted.
        let script = "printf '\\n$ %s\\n\\n' \"$1\"; sh -c \"$1\"; status=$?; \
                      printf '\\nPress Enter to return to ggt. '; read _; exit $status";
        Some(External {
            label: "Upgrade".to_owned(),
            program: "sh".to_owned(),
            args: vec![
                "-c".to_owned(),
                script.to_owned(),
                "sh".to_owned(),
                command.to_owned(),
            ],
            refresh: false,
            upgrade: true,
        })
    }
}

/// Classifies the binary at `exe` (with symlinks resolved). `cargo_home` is
/// where the release installers put it.
pub fn detect(exe: &Path, cargo_home: Option<&Path>, windows: bool) -> Install {
    let text = exe.to_string_lossy().replace('\\', "/");
    if text.contains("/Cellar/git-graph-tui/") {
        return Install::Homebrew;
    }
    let in_cargo_bin = cargo_home.is_some_and(|home| exe.parent() == Some(&*home.join("bin")));
    match (in_cargo_bin, windows) {
        (true, false) => Install::ShellInstaller,
        (true, true) => Install::PowerShellInstaller,
        (false, _) => Install::Unknown,
    }
}

/// How the running binary was installed.
pub fn current(env: &dyn Fn(&str) -> Option<String>) -> Install {
    let Some(exe) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.canonicalize().ok())
    else {
        return Install::Unknown;
    };
    let cargo_home = env("CARGO_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            env("HOME")
                .or_else(|| env("USERPROFILE"))
                .filter(|dir| !dir.is_empty())
                .map(|home| PathBuf::from(home).join(".cargo"))
        })
        .map(|home| home.canonicalize().unwrap_or(home));
    detect(&exe, cargo_home.as_deref(), cfg!(windows))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn homebrew_cellar_paths_are_homebrew() {
        for exe in [
            "/opt/homebrew/Cellar/git-graph-tui/0.6.2/bin/ggt",
            "/usr/local/Cellar/git-graph-tui/0.6.2/bin/ggt",
            "/home/linuxbrew/.linuxbrew/Cellar/git-graph-tui/0.6.2/bin/ggt",
        ] {
            assert_eq!(
                detect(Path::new(exe), None, false),
                Install::Homebrew,
                "{exe}"
            );
        }
    }

    #[test]
    fn the_cargo_bin_directory_is_the_release_installer() {
        let home = Path::new("/home/me/.cargo");
        let exe = Path::new("/home/me/.cargo/bin/ggt");
        assert_eq!(detect(exe, Some(home), false), Install::ShellInstaller);
        let home = Path::new("C:/Users/me/.cargo");
        let exe = Path::new("C:/Users/me/.cargo/bin/ggt.exe");
        assert_eq!(detect(exe, Some(home), true), Install::PowerShellInstaller);
    }

    #[test]
    fn anything_else_is_unknown() {
        let home = Path::new("/home/me/.cargo");
        for exe in [
            "/home/me/src/git-graph-tui/target/release/ggt",
            "/usr/local/bin/ggt",
            "/home/me/.cargo/bin/sub/ggt",
        ] {
            assert_eq!(
                detect(Path::new(exe), Some(home), false),
                Install::Unknown,
                "{exe}"
            );
        }
        assert_eq!(
            detect(Path::new("/home/me/.cargo/bin/ggt"), None, false),
            Install::Unknown
        );
    }

    #[test]
    fn only_homebrew_and_the_shell_installer_upgrade_in_place() {
        assert!(Install::Homebrew.upgrades_in_place());
        assert!(Install::ShellInstaller.upgrades_in_place());
        assert!(!Install::PowerShellInstaller.upgrades_in_place());
        assert!(!Install::Unknown.upgrades_in_place());
        assert_eq!(Install::PowerShellInstaller.upgrade_program(), None);
        assert_eq!(Install::Unknown.upgrade_program(), None);
        assert_eq!(Install::Unknown.command(), None);
    }

    #[test]
    fn the_upgrade_passes_the_command_as_an_argument() {
        let program = Install::Homebrew.upgrade_program().expect("program");
        assert_eq!(program.program, "sh");
        assert_eq!(program.args[0], "-c");
        assert_eq!(program.args[2], "sh");
        assert_eq!(program.args[3], BREW_UPGRADE);
        assert!(program.upgrade);
        assert!(!program.refresh);
    }

    #[test]
    fn the_upgrade_script_runs_the_command_and_keeps_its_exit_status() {
        // Swap in harmless commands for the real ones.
        let mut program = Install::Homebrew.upgrade_program().expect("program");
        for (command, ok) in [("true", true), ("exit 3", false)] {
            program.args[3] = command.to_owned();
            let status = std::process::Command::new(&program.program)
                .args(&program.args)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .status()
                .expect("sh runs");
            assert_eq!(status.success(), ok, "{command}");
        }
    }
}
