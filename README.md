# tui-git-graph

> A Git Graph for the terminal. See your repository's colored commit graph and act on it with right-click menus (checkout, branch, merge, rebase, cherry-pick, reset, push, and more), just like the Git Graph extension for VS Code. A single native binary that works in any terminal, inside [herdr](https://herdr.dev/) panes and inside the [TTT editor](https://tttedit.dev/).

## Concept

Git Graph for VS Code is one of the most loved Git tools because it combines two things in one view:

1. **See** the whole repository at a glance: every local and remote branch, tag, stash, merge and the uncommitted changes, drawn as a clear, color-coded tree.
2. **Act** on it directly: right-click a commit, branch or tag to checkout, merge, rebase, cherry-pick, reset, push, and more.

In the terminal, this combination doesn't exist today. Existing tools either draw a great graph but are read-only ([serie](https://github.com/lusingander/serie), [git-graph](https://github.com/git-bahn/git-graph), [git-igitt](https://github.com/git-bahn/git-igitt)), or manage Git well but with a weaker graph (lazygit, tig). Terminal-first editors and agent workspaces (TTT, Orca, herdr) have the same gap: users keep asking for a "real" Git Graph view.

**tui-git-graph** fills that gap as a standalone terminal app with full mouse support, so it can run anywhere a terminal runs.

## Goals

**Main goal:** offer the capabilities of the Git Graph extension for VS Code, adapted to a terminal UI.

- **Independent.** A single binary whose only runtime dependency is the `git` CLI. No editor, multiplexer or plugin API required.
- **Mouse and keyboard.** Click to select, right-click for context menus, scroll with the wheel. Everything also works from the keyboard.
- **Fast** on real repositories (tens of thousands of commits, many branches): incremental loading, background git calls, a UI that never blocks.
- **Safe.** Every destructive action (reset, delete branch, force push, drop, clean) asks for confirmation and says exactly what will happen.
- **Works where you work.** Any terminal, tmux, herdr panes (via a herdr plugin) and TTT (via a thin TTT plugin that opens it in a terminal tab).
- **Maintainable.** A pure, well-tested core (parsing, layout, actions) with a thin UI layer.

### Non-goals (for now)

- Staging and committing. Use your editor or `git` for that. We show uncommitted changes and offer stash, reset and clean.
- A text editor or a general IDE.
- Version control systems other than Git.
- Reusing code from the Git Graph extension. Everything is written from scratch (see [Licensing](#licensing)).

## Scope: feature parity checklist

Target feature set, based on Git Graph for VS Code. The detailed specs for the first release are in [`odd/tasks/git-graph-mvp.md`](odd/tasks/git-graph-mvp.md).

### Graph view
- [ ] Commit graph with color-coded lanes per branch and merge lines
- [ ] Local and remote branches, tags, stashes and remote HEAD labels, as colored badges
- [ ] Uncommitted changes shown as a virtual row at the top
- [ ] Columns: graph, description, date, author, commit hash (show/hide, adapt to width)
- [ ] Highlight HEAD and mute commits that aren't ancestors of HEAD
- [ ] Incremental loading while scrolling
- [ ] Branch filter: show all, one or several branches, or custom glob patterns
- [ ] Toggle remote branches on/off
- [ ] Find: search by message, author, date, hash, branch or tag name
- [ ] Jump to HEAD, refresh, and fetch from the toolbar

### Actions on branches (right-click a branch badge)
- [ ] Checkout (local, or create a tracking branch from a remote one), create, rename, delete
- [ ] Merge into current branch, rebase current branch onto it, reset
- [ ] Pull, push, fetch into local branch
- [ ] Create archive, copy branch name
- [ ] Create a pull request (GitHub / GitLab / Bitbucket, opens the browser)

### Actions on commits (right-click a commit)
- [ ] Add tag, create branch
- [ ] Checkout (detached), cherry-pick, revert, drop
- [ ] Merge into current branch, rebase current branch on this commit
- [ ] Reset current branch to this commit (soft / mixed / hard)
- [ ] Copy hash / subject to clipboard

### Repository menu ("..." in the toolbar)
- [ ] Pull, push, fetch, "Checkout to..." (pick any branch or tag)
- [ ] Branch, remote, stash and tag submenus; show the git command log

### Actions on tags and stashes
- [ ] Add, delete and push tags; view annotated tag details
- [ ] Apply, pop, drop stashes; create branch from stash

### Actions on uncommitted changes
- [ ] Stash, reset, clean

### Commit details
- [ ] Click a commit to expand its metadata (hash, parents, author, committer, date) and changed files
- [ ] View the diff of any file (built-in viewer, or your configured `git difftool` / pager such as delta)
- [ ] View a file at that revision
- [ ] Compare any two commits, or a commit with the working tree

### Repository settings
- [ ] Manage remotes (add, remove, fetch, prune)
- [ ] Issue linking (turn `#123` into links that open in the browser)

### Stretch goals (inspired by Git Graph+)
- [ ] Interactive rebase UI (reorder, squash, fixup, drop)
- [ ] Conflict detection banner with continue / abort
- [ ] Worktree management
- [ ] Reflog view with recovery of lost commits

## Tech stack

| Area | Choice | Why |
| --- | --- | --- |
| Language | **Rust** (stable, 2024 edition) | A single fast binary. The best reference code for graph layout (git-graph, serie) is Rust. |
| TUI | **[ratatui](https://ratatui.rs/)** + **crossterm** | De facto Rust TUI stack. Full mouse support (click, right-click, wheel), colors, cross-platform. |
| Git access | **The `git` CLI** (`std::process::Command`, `-z` / `%x1f` machine-readable formats) | Exactly the same behavior as the user's terminal: config, hooks, credential helpers, SSH, signing. No libgit2 differences. |
| Concurrency | Worker threads + channels (no async runtime) | Git calls run off the UI thread and can be cancelled. Simple and enough for a process-driven app. |
| CLI | `clap` | `tui-git-graph [PATH]`, flags for branch filter, all/remote branches. |
| Config | TOML at `~/.config/tui-git-graph/config.toml` (`serde`, `toml`) | Columns, date format, colors, keybindings, default filters. |
| Clipboard | OSC 52 (works over SSH, tmux, herdr), `arboard` fallback | Copy hash, subject, branch name. |
| Errors | `thiserror` in the core, `anyhow` in the binary | Typed errors where we match on them, context where we report them. |
| Testing | `cargo nextest`, `insta` snapshots, ratatui `TestBackend`, fixture repos built in temp dirs | Layout and rendering are snapshot-tested. Actions run against real throwaway repos. |
| Quality | `rustfmt`, `clippy -D warnings`, `cargo deny` | Enforced in CI. |
| Distribution | GitHub Releases (macOS, Linux; Windows later), Homebrew tap, `cargo install` | One binary, no runtime. |

## Architecture

A Cargo workspace with a pure core and a thin UI. The core never touches the terminal, and the UI never runs git directly.

```
crates/
  core/                 # library: no terminal, no threads; 100% unit-testable
    git/                # command builder (-C <repo>, safe argv), output parsers (log, refs, status, stash)
    model/              # Commit, Ref, Repo snapshot, filters, search
    graph/              # lane layout: commits -> rows of lanes/edges (incremental, stable across pages)
    actions/            # one type per action: validation, git argv, confirmation text, refresh scope
  app/                  # binary `tui-git-graph`
    runner/             # executes core commands on worker threads; cancel; stale-result guard
    state/              # single AppState; all updates go through an Event -> update() function
    ui/                 # ratatui widgets: graph, badges, details pane, popup menus, dialogs, toolbar
    input/              # key + mouse -> Event; hit-testing from the last rendered layout
integrations/
  herdr/                # herdr-plugin.toml: opens tui-git-graph in a herdr pane
  ttt/                  # tiny TTT Lua plugin: "Git Graph: Open" runs tui-git-graph in a terminal tab
tests/fixtures/         # scripts that build fixture repos (branches, merges, tags, stash, remotes)
```

**Data flow (unidirectional):**

```
input (key/mouse) ─▶ Event ─▶ update(state) ─▶ Command ─▶ runner (git, worker thread)
       ▲                                                          │
       └──────── render(state) ◀── update(state) ◀── Event::GitResult
```

- **Rendering is a pure function of state.** Each frame records a hit-map (which cell belongs to which row, badge or menu item), so mouse clicks and right-clicks resolve against exactly what was drawn.
- **Lane layout is incremental.** Loading more commits never changes rows already shown.
- **Actions are data.** Each action declares its git argv, whether it is destructive, its confirmation text and what to refresh. Menus are built from the selected target (commit, branch, tag, stash, uncommitted row), so the same action list drives mouse menus, keyboard menus and tests.

## Roadmap

1. **MVP:** colored graph with badges, uncommitted row, selection, scrolling, mouse + keyboard, branch and commit context menus with checkout / switch branch, create branch, tag, cherry-pick, revert, merge, reset, copy; commit details with file diffs.
2. **Daily driver:** branch filter, remote toggle, search, repository "..." menu, pull / push / fetch, tags and stashes, config file.
3. **Parity:** compare commits, archive, remotes management, issue linking, PR creation.
4. **Integrations and release:** herdr plugin, TTT plugin, Homebrew tap, GitHub Releases.
5. **Beyond:** interactive rebase, conflict handling, worktrees, reflog.

## Licensing

This project is MIT licensed. It is **inspired by** Git Graph for VS Code but contains **no code from it**; the Git Graph extension uses its own non-standard license. All implementation is original. MIT/Apache-2.0 projects listed below may be studied; anything substantial adapted from them is credited.

## Inspiration

| Project | Why it matters |
| --- | --- |
| [mhutchie/vscode-git-graph](https://github.com/mhutchie/vscode-git-graph) | The reference experience: graph plus actions from right-click menus. Our feature parity target. |
| [lusingander/serie](https://github.com/lusingander/serie) | Rust + ratatui git graph TUI (MIT). The closest existing tool: excellent rendering, read-only. Reference for drawing and terminal image/glyph handling. |
| [git-bahn/git-graph](https://github.com/git-bahn/git-graph) | Rust CLI (MIT) with a very clean lane layout per branching model. Reference for the graph layout algorithm. |
| [git-bahn/git-igitt](https://github.com/git-bahn/git-igitt) | Interactive TUI based on git-graph (MIT). Reference for browsing history and diffs in a terminal. |
| [the0807/git-graph-plus](https://github.com/the0807/git-graph-plus) | A modern, from-scratch Git Graph for VS Code (Apache-2.0). Ideas for stretch goals: interactive rebase, drag to merge, conflict prediction, reflog, worktrees. |
| [stablyai/orca#4521](https://github.com/stablyai/orca/issues/4521) | Feature request in the Orca agent IDE asking for exactly this. Shows real demand from users of terminal and agent-first tools. |
| [herdr](https://herdr.dev/) · [TTT editor](https://tttedit.dev/) | Host environments we integrate with through thin plugins. |
