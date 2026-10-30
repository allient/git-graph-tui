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
```

- **`[layout]`** is written for you when you quit after dragging a divider, hiding a column or picking a View & Sort choice. Only that table is rewritten; the rest of the file keeps its comments.
- **`[diff] pager`** is a shell command, so `"delta --side-by-side"` or `"less -R"` work too.
- **`[issues]`**: without `url`, `#123` opens issue 123 on origin's GitHub, GitLab or Bitbucket page.

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
git-graph-tui [PATH] [--icons auto|nerd|unicode]
```

`PATH` is any folder inside the repository (the current folder by default). `--icons` wins over the variable and the config file.

## Icons

With `auto` (the default), git-graph-tui uses VS Code's icons (Codicons) in Ghostty, WezTerm and Kitty, which ship the Nerd Font symbols, also inside tmux or herdr. Elsewhere it draws plain Unicode symbols (`[↻]` Refresh, `[⇣]` Fetch, `[...]` menu).

To get the icons in another terminal:

1. Install a [Nerd Font](https://www.nerdfonts.com/), for example `brew install --cask font-jetbrains-mono-nerd-font`.
2. Select it in your terminal's settings.
3. Run with `--icons nerd`, or `export GIT_GRAPH_TUI_ICONS=nerd`.

If the icons show as empty boxes, the terminal isn't using a Nerd Font: use `--icons unicode`.
