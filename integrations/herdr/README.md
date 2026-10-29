# tui-git-graph for herdr

A [herdr](https://herdr.dev/) plugin that opens tui-git-graph in a herdr tab, for the repository you're working in. Plain right-click opens tui-git-graph's context menus there, not herdr's pane menu.

## Install

The plugin is a thin launcher: install tui-git-graph itself first, from a checkout of this repository:

```sh
cargo install --path crates/app
```

Then install the plugin from GitHub:

```sh
herdr plugin install allient/tui-git-graph/integrations/herdr
```

Or link a local checkout while working on the plugin:

```sh
herdr plugin link /path/to/tui-git-graph/integrations/herdr
```

The plugin needs herdr 0.9.5 or later, on macOS or Linux.

## Open it

Bind a key in herdr's `config.toml` (`prefix+G` here; any free key works):

```toml
[[keys.command]]
key = "prefix+G"
type = "plugin_action"
command = "tui-git-graph.open"
description = "open the Git Graph"
```

Then run `herdr server reload-config`. You can also run the action from the command line:

```sh
herdr plugin action invoke tui-git-graph.open
```

tui-git-graph opens in a new tab, in the first of these that herdr knows:

1. the active worktree's checkout,
2. the focused pane's directory,
3. the workspace's directory,
4. the plugin's own directory, as a last resort.

Quit tui-git-graph (`q`) to close the tab.

If herdr can't find the `tui-git-graph` binary, the tab says so and waits for Enter. The plugin looks for it on the herdr server's `PATH`, then in `~/.cargo/bin`.

## Right-click

herdr sends plain right-click to its own pane menu by default, which hides tui-git-graph's menus. herdr can route it to the app instead, one pane at a time, with `herdr pane input --right-click pane`:

- The plugin's tab does this before it starts tui-git-graph.
- tui-git-graph does it too whenever it runs inside herdr, even when you start it yourself in a pane. It gives the click back to herdr when it quits. Set `TUI_GIT_GRAPH_HERDR_RIGHT_CLICK=off` to turn that off.

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
- `scripts/run.sh`: the pane. Routes right-click to the pane, changes to `TGG_TARGET_DIR` and starts `tui-git-graph` (or `$TGG_BIN`).
- `tests/run.sh`: tests for both scripts, with herdr and tui-git-graph stubbed. Run `sh integrations/herdr/tests/run.sh`.

## For the root README

When this is merged, the root README should say:

- In the features list, replace "herdr panes (via a herdr plugin)" with: "herdr (a [herdr plugin](integrations/herdr/README.md) opens it in a tab; plain right-click works inside herdr)".
- In the layout tree, replace `integrations/herdr/     # herdr-plugin.toml: opens tui-git-graph in a herdr pane (planned)` with `integrations/herdr/     # herdr plugin: opens tui-git-graph in a herdr tab`.
- In the roadmap's step 4, mark the herdr plugin done.
- Under the right-click or mouse notes: "Inside herdr, tui-git-graph routes plain right-click to itself while it runs (`herdr pane input --right-click pane`) and gives it back on exit. Set `TUI_GIT_GRAPH_HERDR_RIGHT_CLICK=off` to keep herdr's menu."
