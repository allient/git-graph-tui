# Git Graph MVP

Colored commit graph TUI (Rust + ratatui) with VS Code Git Graph-style context menus, checkout and branch switching. Runs standalone in any terminal. Source of truth for the specs: the user's screenshots of VS Code Git Graph (2026-10-28).

## Specs

- **S1. Launch.** `tui-git-graph [PATH]` opens the graph for the repository containing PATH (default: current directory). Outside a repo it exits with a clear error. `q` quits.
- **S2. Toolbar.** "Branches:" dropdown, default "Show All", to filter by one or more branches. A "Show Remote Branches" checkbox. Refresh and Fetch actions.
- **S3. Columns.** Graph | Description | Date | Author | Commit. Graph and Description are always visible. The others hide when the tab is narrow.
- **S4. Colored lanes.** Each branch lane has its own color from a palette that works on dark and light themes. Merge and branch-off edges are drawn between lanes. A filled node is a commit, and a hollow node is HEAD or the uncommitted row.
- **S5. Uncommitted Changes row.** A virtual top row "Uncommitted Changes (N)" (N = changed files), linked to HEAD, shown only when the working tree is dirty.
- **S6. Ref badges** before the subject: a branch badge with the name, plus an "origin" segment when the remote branch is on the same commit. Separate `origin/HEAD` and remote-only badges. Tag badges. The badge color matches the lane color. The current branch (HEAD) is highlighted.
- **S7. Muted commits.** Commits that aren't ancestors of HEAD are drawn muted, e.g. "Merge branch 'develop'".
- **S8. Branch context menu** (right-click a branch badge):
  - Checkout Branch (only when it isn't the current branch). For a remote-only branch, create a local tracking branch.
  - Rename Branch...
  - Delete Branch... (not on the current branch; confirm, offer force for unmerged)
  - Merge into current branch...
  - Rebase current branch on Branch...
  - Push Branch...
  - separator
  - Create Archive
  - Select / Unselect in Branches Dropdown
  - separator
  - Copy Branch Name to Clipboard
- **S9. Commit context menu** (right-click a commit row):
  - Add Tag...
  - Create Branch...
  - separator
  - Checkout... (detached HEAD, confirm)
  - Cherry Pick...
  - Revert...
  - Drop... (confirm)
  - separator
  - Merge into current branch...
  - Rebase current branch on this Commit...
  - Reset current branch to this Commit... (soft / mixed / hard, confirm)
  - separator
  - Copy Commit Hash to Clipboard
  - Copy Commit Subject to Clipboard
- **S10. Commit details.** Clicking a commit expands an inline panel under it: Commit hash, Parents (select one to jump to it), Author, Committer, Date. A changed-files tree with +/- counts on the right. Selecting a file opens its diff in the built-in diff view (or the configured pager/difftool).
- **S11. Repository "..." menu** (from the VS Code Source Control menu screenshot): Pull, Push, Fetch, Checkout to... (pick any branch or tag), and submenus for Branch, Remote, Stash and Tags, plus Show Git Output.
- **S12. Branch switching.** Checking out any local or remote branch, or any commit, from the menus or "Checkout to..." updates HEAD, refreshes the graph, keeps the selection on the same commit, and reports success or the git error (e.g. uncommitted changes would be overwritten).
- **S13. Safety.** Every destructive action (delete branch, drop, hard reset, force push, detached checkout) shows a confirmation dialog first that names exactly what will happen.
- **S14. Keyboard.** Every mouse action also works from the keyboard: j/k or arrows to move, Enter for details, a menu key for the context menu, Escape to close, PgUp/PgDn and g/G.

## Tasks

| ID | Task | Specs | Route | Status | Commit |
| --- | --- | --- | --- | --- | --- |
| T1 | Cargo workspace (7 crates), toolchain, lints, deny, nextest, CI | — | inline | done | (this branch) |
| T2 | Fixture repo builders for tests (branches, merges, tags, stash, bare `origin`) | all | inline | open | |
| T3 | Git command layer: repo discovery, safe argv, runner on worker threads, timeouts, cancel, stale-result guard | S1, S12 | inline | open | |
| T4 | Parsers: log (`%x1f` + NUL), refs, status, stash | S3, S5, S6 | inline | open | |
| T5 | Lane layout (incremental, stable across pages) + muted non-ancestors | S4, S7 | inline | open | |
| T6 | Graph rendering: lanes, nodes, badges, columns + hit-map | S3–S7 | inline | open | |
| T7 | App loop: state, events, scrolling, selection, keyboard | S14 | inline | open | |
| T8 | Mouse: click select, right-click target, wheel (via hit-map) | S8, S9, S10 | inline | open | |
| T9 | Popup menu + confirmation/input dialogs | S8, S9, S11, S13 | inline | open | |
| T10 | Branch menu actions + checkout / switch | S8, S12, S13 | inline | open | |
| T11 | Commit menu actions | S9, S13 | inline | open | |
| T12 | Inline commit details + diff view | S10 | inline | open | |
| T13 | Toolbar: branch filter, remote toggle, refresh, fetch | S2 | inline | open | |
| T14 | Repository "..." menu | S11 | inline | open | |

## Log

- **L1** (user, verbatim): "I want a clone ad gitgraph also have thsi also have menus [Image #2] [Image #3] [Image #4] be able to checkout and change to otehr branches ..."
- **L2** Decision: the graph view uses raw cells, not the `table` widget. On v1.9.0 the table takes plain-string rows (no colors) and one `node_menu` shared by every row, so it can't produce S4, S6 or separate S8/S9 menus. `ttt.open_dialog` is always centered, so context menus are drawn as a raw-cell popup (T8).
- **L3** Spike (v1.9.0, headless): raw cells with theme styles, `sys.exec_async("git", ...)`, and key and mouse events (left, none/release, key + rune) all work. **Mouse x/y are screen-absolute** (`internal/plugin/panel_widget.go` forwards raw tcell events), and the panel proxy only exposes `size()`, so the plugin can't map a click to a panel cell. This blocks T7.
- **L4** Spike: on v1.9.0, `--plugin FILE` grants every permission *except* `system.exec` and ignores the manifest. E2E tests must install the plugin into a temp `TTT_CONFIG_DIR/plugins/` with a pre-approved `plugins.ttt.json` (same as `ttt-pdf/tests/run.sh`).
- **L5** S11 vs the README non-goal: the repository menu covers git operations (pull, push, fetch, checkout, branch, remote, stash, tags). Staging and committing stay out of scope (README non-goal).
- **L6** Pivot (2026-10-28): the project moved from a TTT Lua plugin to a standalone native TUI, **Rust + ratatui + crossterm**, calling the `git` CLI. Reasons: L3 (no panel-relative mouse coordinates in TTT plugins), the table widget limits (L2), the slow Lua 5.1 interpreter, the awkward headless testing (L4), and independence from a young plugin API. herdr and TTT become thin launchers in `integrations/`. The repo was renamed `ttt-git-graph` → `tui-git-graph`. L2–L4 are kept as history. Specs S1–S14 are unchanged, except that S1, S4, S10 and S13 no longer mention TTT.
- **L7** T1 done: workspace `crates/{model,git,graph,actions,ui,app,testkit}`, dependency catalog in root `[workspace.dependencies]`, pedantic workspace lints, rust-toolchain 1.95 (MSRV 1.88), cargo-deny, nextest, CI (fmt, clippy, machete, tests on Linux + macOS, MSRV, deny). Evidence: `cargo fmt --check`, `cargo lint`, `cargo t` (3 passed), `cargo machete`, `cargo deny check`, and `cargo +1.88 check` all pass locally.
