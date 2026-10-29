# git-graph-tui for herdr

A [herdr](https://herdr.dev/) plugin that opens git-graph-tui in a herdr tab, for the repository you're working in. Plain right-click opens git-graph-tui's context menus there, not herdr's pane menu.

## Install

The plugin is a thin launcher: install git-graph-tui itself first, from a checkout of this repository:

```sh
cargo install --path crates/app
```

Then install the plugin from GitHub:

```sh
herdr plugin install allient/git-graph-tui/integrations/herdr
```

Or link a local checkout while working on the plugin:

```sh
herdr plugin link /path/to/git-graph-tui/integrations/herdr
```

The plugin needs herdr 0.9.5 or later, on macOS or Linux.

## Open it

Bind a key in herdr's `config.toml` (`prefix+G` here; any free key works):

```toml
[[keys.command]]
key = "prefix+G"
type = "plugin_action"
command = "git-graph-tui.open"
description = "open the Git Graph"
```

Then run `herdr server reload-config`. You can also run the action from the command line:

```sh
herdr plugin action invoke git-graph-tui.open
```

git-graph-tui opens in a new tab, in the first of these that herdr knows:

1. the active worktree's checkout,
2. the focused pane's directory,
3. the workspace's directory,
4. the plugin's own directory, as a last resort.

Quit git-graph-tui (`q`) to close the tab.

If herdr can't find the `git-graph-tui` binary, the tab says so and waits for Enter. The plugin looks for it on the herdr server's `PATH`, then in `~/.cargo/bin`.

## Right-click

herdr sends plain right-click to its own pane menu by default, which hides git-graph-tui's menus. herdr can route it to the app instead, one pane at a time, with `herdr pane input --right-click pane`:

- The plugin's tab does this before it starts git-graph-tui.
- git-graph-tui does it too whenever it runs inside herdr, even when you start it yourself in a pane. It gives the click back to herdr when it quits. Set `GIT_GRAPH_TUI_HERDR_RIGHT_CLICK=off` to turn that off.

Right-clicking the pane's frame still opens herdr's menu.

If right-click still opens herdr's menu (for example on an older herdr), there are two fallbacks:

- Press `m` (or the Menu key, or Shift+F10) to open the menu for the selected row.
- Let a modifier through for every pane, in herdr's `config.toml`, then use Ctrl+right-click:

  ```toml
  [ui]
  right_click_passthrough_modifier = "ctrl"
  ```

## Files

- `herdr-plugin.toml`: the manifest. An `open` action and a `graph` pane.
- `scripts/open.sh`: the action. Actions run without a terminal, so it picks the directory from herdr's context and opens the `graph` pane, passing the directory as `TGG_TARGET_DIR`.
- `scripts/run.sh`: the pane. Routes right-click to the pane, changes to `TGG_TARGET_DIR` and starts `git-graph-tui` (or `$TGG_BIN`).
- `tests/run.sh`: tests for both scripts, with herdr and git-graph-tui stubbed. Run `sh integrations/herdr/tests/run.sh`.
