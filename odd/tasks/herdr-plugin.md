# herdr plugin and plain right-click inside herdr

Objective: open tui-git-graph from herdr with a plugin, and make plain right-click reach the app's context menus inside a herdr pane.
Branch: feat/herdr-plugin (worktree tui-git-graph-herdr), from origin/main; committed locally, not pushed (the user decides the PR). Delivery: ask-on-risk. Tests: `cargo t` (nextest) and `sh integrations/herdr/tests/run.sh`.

## Specs

- **S1** "I have notcied right click do nto work when I use on a cli on herdr". herdr sends plain right-click to its own pane menu by default. Fix: set the pane's routing with `herdr pane input --right-click pane` (as TTT's herdr plugin does); `[ui] right_click_passthrough_modifier` stays an optional fallback.
- **S2** "can you plan create a plugin for herdr". `integrations/herdr/`: `herdr-plugin.toml`, `scripts/open.sh` (actions run headless: re-dispatch to `plugin pane open`, target directory through `--env TGG_TARGET_DIR`, picked from `checkout_path`, `focused_pane_cwd`, `workspace_cwd`, then `.`), `scripts/run.sh` (pane entrypoint: route right-click to the pane only when `HERDR_PLUGIN_ENTRYPOINT_ID` is set, cd, exec `tui-git-graph`; a missing binary shows a hint and waits for a key), `README.md` (install with `plugin install`/`plugin link`, a keybinding snippet, the modifier fallback, the root README lines to add).
- **S3** The app, when `HERDR_ENV=1` and `HERDR_PANE_ID` are set: best effort `$HERDR_BIN_PATH pane input --pane $HERDR_PANE_ID --right-click pane` on startup and `--right-click herdr` on exit (Drop guard next to `MouseCapture`). Errors are ignored and never block startup. Off switch: `TUI_GIT_GRAPH_HERDR_RIGHT_CLICK=off` (assumption: an env var, since the config file is outside this change's edit surface).
- **S4** Tests: a POSIX sh test with a stubbed `HERDR_BIN_PATH` that records argv; Rust unit tests for the guard with an injected runner (no herdr in CI); plain right-click checked by hand in herdr when possible.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S1, S3, S4 | inline | done | feat(app): route plain right-click to the app inside herdr |
| T2 | S1, S2, S4 | inline | done | feat(herdr): add a herdr plugin that opens tui-git-graph in a tab |

Route evidence: two small units (~150 lines each) that a single writer finishes faster than starting workers; inline.

## Log

- **L1** (2026-10-29) "can you plan create a plugin for herdr I have notcied right click do nto work when I use on a cli on herdr plan thsi after you fnishe or ask @w2".
- **L2** (2026-10-29) Planning (w2): herdr 0.9.5 `herdr pane input --help` shows `--right-click <TARGET> [possible values: herdr, pane]`; the socket schema has `PaneInputSetParams{pane_id, right_click}`, default `herdr`. TTT's plugin (eugenioenko/ttt `herdr-plugin/scripts/open-worktree.sh`) runs `pane input --current --right-click pane` before `exec ttt`. `herdr pane get` doesn't report the routing, so the app can't read the previous value back; on exit it restores herdr's default.
- **L3** (2026-10-29) "if needed ask for help to @w2 so it work on herdr plugin". Build delegated to w2: own worktree, allowed surfaces `integrations/herdr/**`, `crates/app/src/main.rs`, `crates/app/src/herdr.rs`, this document; the root README is updated by the other session at merge time.
- **L4** (2026-10-29) T1 done: `crates/app/src/herdr.rs` `RightClick` guard (injected runner, 7 unit tests), wired in `main.rs` next to `MouseCapture`. Gate green, 765 tests.
- **L5** (2026-10-29) T2 done: `integrations/herdr/` manifest, `open.sh`, `run.sh`, README, `tests/run.sh` (12 checks, pass under sh and dash). run.sh doesn't `exec` the app, so a failure (not a repository) stays on screen until Enter. Checked in herdr 0.9.5: `herdr plugin link` accepted the manifest; `plugin action invoke tui-git-graph.open` opened a tab in the focused pane's directory showing the install hint (the binary isn't installed), closed by Enter; `plugin pane open … --env TGG_BIN=target/debug/tui-git-graph` ran the graph in a herdr tab. Not checked: a real plain right-click (needs a mouse), and `checkout_path` (no worktree context in that invocation). `tests/run.sh` isn't in CI yet (.github is outside this change).
