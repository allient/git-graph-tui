---
title: Installation
description: Install git-graph-tui on macOS, Linux or Windows.
---

Prebuilt binaries for macOS (Apple Silicon and Intel), Linux (x86_64 and arm64) and Windows are attached to every [GitHub Release](https://github.com/allient/git-graph-tui/releases). git-graph-tui needs `git` on your `PATH`.

## Homebrew (macOS, Linux)

```sh
brew install allient/tap/git-graph-tui
```

## Shell installer (macOS, Linux)

The installer puts the binary in `~/.cargo/bin`:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/allient/git-graph-tui/releases/latest/download/git-graph-tui-installer.sh | sh
```

## PowerShell (Windows)

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/allient/git-graph-tui/releases/latest/download/git-graph-tui-installer.ps1 | iex"
```

## Manual download

Download the archive for your platform from the [release page](https://github.com/allient/git-graph-tui/releases/latest), check it against its `.sha256` file, and put `ggt` on your `PATH`.

## From source

You need Rust (the toolchain is pinned in `rust-toolchain.toml`; `rustup` installs it automatically) and `git`:

```sh
git clone https://github.com/allient/git-graph-tui.git
cd git-graph-tui
cargo install --path crates/app
```

## Recommended: a Nerd Font

For the best experience, use a terminal font with the [Nerd Font](https://www.nerdfonts.com/) symbols. git-graph-tui then draws VS Code's own icons (Codicons) in the toolbar, the Source Control pane, the file tree and the ref badges. Without one, it falls back to plain Unicode symbols.

In Ghostty, WezTerm and Kitty the icons turn on by themselves. In any other terminal, install a Nerd Font, select it in the terminal's settings and run `ggt --icons nerd` (or set `icons = "nerd"` in the config file). [Icons](/reference/configuration/#icons) has the step-by-step setup for each terminal.

## Check it works

```sh
ggt --version
```

The command is `ggt`. Before 0.3.0 it was called `git-graph-tui`.
