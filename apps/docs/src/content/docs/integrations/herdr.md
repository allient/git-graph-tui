---
title: herdr
description: Open git-graph-tui in a herdr tab, with plain right-click.
---

A [herdr](https://herdr.dev/) plugin opens git-graph-tui in a herdr tab, for the repository you're working in. Plain right-click opens git-graph-tui's menus there, not herdr's pane menu.

## Install

The plugin is a thin launcher, so [install git-graph-tui](/getting-started/installation/) first. Then install the plugin (herdr 0.9.5 or later, macOS or Linux):

```sh
herdr plugin install allient/git-graph-tui/integrations/herdr
```

## Open it

Bind a key in herdr's `config.toml` (`prefix+G` here; any free key works):

```toml
[[keys.command]]
key = "prefix+G"
type = "plugin_action"
command = "git-graph-tui.open"
description = "open the Git Graph"
```

Then run `herdr server reload-config`, or run the action directly:

```sh
herdr plugin action invoke git-graph-tui.open
```

git-graph-tui opens in a new tab, in the active worktree's checkout (or the focused pane's directory, or the workspace's). Quit it with `q` to close the tab.

## Right-click

herdr sends plain right-click to its own pane menu by default. git-graph-tui routes it to itself while it runs inside herdr (`herdr pane input --right-click pane`), even when you start it yourself in a pane, and gives it back when it quits. Set `GIT_GRAPH_TUI_HERDR_RIGHT_CLICK=off` to keep herdr's menu.

If right-click still opens herdr's menu (for example on an older herdr), press `m` for the selected row's menu, or let `Ctrl+right-click` through in herdr's `config.toml`:

```toml
[ui]
right_click_passthrough_modifier = "ctrl"
```
