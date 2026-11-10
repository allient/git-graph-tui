---
title: Configuration
description: The config file, environment variables and command-line flags.
---

## Config file

Settings live in `~/.config/git-graph-tui/config.toml` (under `$XDG_CONFIG_HOME` when it is set, or wherever `GIT_GRAPH_TUI_CONFIG` points). The file is optional.

```toml
icons = "auto"            # "auto", "nerd" or "unicode"

[layout]                  # saved for you when you quit after resizing
source_control_width = 36
graph_column = 12
date_column = 16
author_column = 14
hash_column = 8
hidden_columns = ["author"] # "date", "author", "commit"
changes_view = "tree"       # Source Control files: "list" (default) or "tree"
changes_sort = "name"       # "path" (default), "name" or "status"

[diff]
pager = "delta"           # pipe file diffs into this command; unset or "" uses the built-in view

[issues]                  # where issue references in commit messages link to
url = "https://jira.example.com/browse/PROJ-$1"   # $1 is replaced by the number
prefix = "PROJ-"          # what starts a reference; default "#"
[refresh]                 # pick up changes made outside the app
watch = true              # reload when git's own files change (default true)
status_interval = 3       # seconds between working-tree checks while the terminal has focus; 0 turns them off
```

- **`[layout]`** is written for you when you quit after dragging a divider, hiding a column or picking a View & Sort choice. Only that table is rewritten; the rest of the file keeps its comments.
- **`[diff] pager`** is a shell command, so `"delta --side-by-side"` or `"less -R"` work too.
- **`[issues]`**: without `url`, `#123` opens issue 123 on origin's GitHub, GitLab or Bitbucket page.
- **`[refresh]`**: commits, checkouts and fetches made elsewhere show up within about a second (the app watches git's own files, not your working tree). Edited files are found with `git status` while the terminal has focus.

A mistake in the file (an unknown setting or value) stops the app with a message naming the file and the line.

## Environment variables

| Variable | Does |
| --- | --- |
| `GIT_GRAPH_TUI_CONFIG` | Path of the config file |
| `GIT_GRAPH_TUI_ICONS` | `auto`, `nerd` or `unicode`; wins over the config file |
| `GIT_GRAPH_TUI_PAGER` | Diff pager; wins over `[diff] pager`, and an empty value turns it off |
| `GIT_GRAPH_TUI_HERDR_RIGHT_CLICK` | `off` keeps herdr's own right-click menu (see [herdr](/integrations/herdr/)) |

## Command line

```sh
ggt [PATH] [--icons auto|nerd|unicode]
```

`PATH` is any folder inside the repository (the current folder by default). `--icons` wins over the variable and the config file.

## Icons

A [Nerd Font](https://www.nerdfonts.com/) gives the best experience: git-graph-tui draws VS Code's icons (Codicons) in the toolbar, the Source Control pane, the file tree and the ref badges, which get rounded ends. Without one, it draws plain Unicode symbols (`[↻]` Refresh, `[⇣]` Fetch, `[...]` menu) and square badges.

With `auto` (the default), the icons turn on in Ghostty, WezTerm and Kitty, which ship the Nerd Font symbols, also inside tmux or herdr. Any other terminal can't report its font, so the icons stay off until you turn them on:

1. Install a Nerd Font, for example JetBrains Mono:
   - macOS: `brew install --cask font-jetbrains-mono-nerd-font`
   - Linux: download it from [nerdfonts.com/font-downloads](https://www.nerdfonts.com/font-downloads), unzip it into `~/.local/share/fonts` and run `fc-cache -f`.
   - Windows: download it from the same page, then right-click the `.ttf` files and choose **Install**.
2. Select it as your terminal's font, for example "JetBrainsMono Nerd Font":
   - iTerm2: Settings → Profiles → Text → Font.
   - Terminal.app: Settings → Profiles → Text → Font → Change.
   - VS Code: `"terminal.integrated.fontFamily": "JetBrainsMono Nerd Font"`.
   - Windows Terminal: Settings → your profile → Appearance → Font face.
   - Alacritty: `[font.normal] family = "JetBrainsMono Nerd Font"` in `alacritty.toml`.
3. Turn the icons on: `ggt --icons nerd` for one run, `export GIT_GRAPH_TUI_ICONS=nerd` in `~/.zshrc` or `~/.bashrc`, or `icons = "nerd"` in the config file.

If the icons show as empty boxes or question marks, the terminal isn't using a Nerd Font: select it again, or use `--icons unicode`.
