# Architecture

How git-graph-tui is built and why. For the user-facing overview see the [README](../README.md); for day-to-day development see [CONTRIBUTING.md](../CONTRIBUTING.md).

## Concept

Git Graph for VS Code is one of the most loved Git tools because it combines two things in one view:

1. **See** the whole repository at a glance: every local and remote branch, tag, stash, merge and the uncommitted changes, drawn as a clear, color-coded tree.
2. **Act** on it directly: right-click a commit, branch or tag to checkout, merge, rebase, cherry-pick, reset, push, and more.

In the terminal, this combination doesn't exist today. Existing tools either draw a great graph but are read-only ([serie](https://github.com/lusingander/serie), [git-graph](https://github.com/git-bahn/git-graph), [git-igitt](https://github.com/git-bahn/git-igitt)), or manage Git well but with a weaker graph (lazygit, tig). Terminal-first editors and agent workspaces (TTT, Orca, herdr) have the same gap: users keep asking for a "real" Git Graph view.

**git-graph-tui** fills that gap as a standalone terminal app with full mouse support, so it can run anywhere a terminal runs.

## Goals

**Main goal:** offer the capabilities of the Git Graph extension for VS Code, adapted to a terminal UI.

- **Independent.** A single binary whose only runtime dependency is the `git` CLI. No editor, multiplexer or plugin API required.
- **Mouse and keyboard.** Click to select, right-click for context menus, scroll with the wheel. Everything also works from the keyboard. Inside herdr, git-graph-tui routes plain right-click to itself while it runs (`herdr pane input --right-click pane`) and gives it back on exit; set `GIT_GRAPH_TUI_HERDR_RIGHT_CLICK=off` to keep herdr's menu.
- **Fast** on real repositories (tens of thousands of commits, many branches): incremental loading, background git calls, a UI that never blocks.
- **Safe.** Every destructive action (reset, delete branch, force push, drop, clean) asks for confirmation and says exactly what will happen.
- **Works where you work.** Any terminal, tmux, herdr (a [herdr plugin](../integrations/herdr/README.md) opens it in a tab; plain right-click works inside herdr) and TTT (run it in a terminal tab).
- **Maintainable.** A pure, well-tested core (parsing, layout, actions) with a thin UI layer.

### Non-goals (for now)

- A full editor for commits: the Source Control pane stages, discards and commits (also amend and sign-off) with a short message; for long messages or partial staging (hunks), use your editor or `git`.
- A text editor or a general IDE.
- Version control systems other than Git.
- Reusing code from the Git Graph extension. Everything is written from scratch (see [License](../README.md#license)).

## Tech stack

| Area | Choice | Why |
| --- | --- | --- |
| Language | **Rust** (stable, 2024 edition) | A single fast binary. The best reference code for graph layout (git-graph, serie) is Rust. |
| TUI | **[ratatui](https://ratatui.rs/)** + **crossterm** | De facto Rust TUI stack. Full mouse support (click, right-click, wheel), colors, cross-platform. |
| Git access | **The `git` CLI** (`std::process::Command`, `-z` / `%x1f` machine-readable formats) | Exactly the same behavior as the user's terminal: config, hooks, credential helpers, SSH, signing. No libgit2 differences. |
| Concurrency | Worker threads + channels (no async runtime) | Git calls run off the UI thread and can be cancelled. Simple and enough for a process-driven app. |
| CLI | `clap` | `ggt [PATH]`, flags for branch filter, all/remote branches. |
| Config | TOML at `~/.config/git-graph-tui/config.toml` (`serde`, `toml`) | Columns, date format, colors, keybindings, default filters. |
| Clipboard | OSC 52 (works over SSH, tmux, herdr), `arboard` fallback | Copy hash, subject, branch name. |
| Errors | `thiserror` in the core, `anyhow` in the binary | Typed errors where we match on them, context where we report them. |
| Testing | `cargo nextest`, `insta` snapshots, ratatui `TestBackend`, fixture repos built in temp dirs | Layout and rendering are snapshot-tested. Actions run against real throwaway repos. |
| Quality | `rustfmt`, `clippy -D warnings`, `cargo deny` | Enforced in CI. |
| Distribution | GitHub Releases (macOS, Linux, Windows) built by [cargo-dist](https://opensource.axo.dev/cargo-dist/), shell and PowerShell installers, Homebrew tap, `cargo install --path` | One binary, no runtime. |

## Architecture

A Cargo workspace (monorepo) of small crates with one-way dependencies. Pure crates never touch the terminal, and the UI never runs git directly.

| Crate | Path | Role | May depend on |
| --- | --- | --- | --- |
| `ggt-model` | `crates/model` | Domain types: commit ids, commits, refs, signatures. Pure data. | — |
| `ggt-git` | `crates/git` | Safe git argv building, process execution behind a trait, output parsers | model |
| `ggt-graph` | `crates/graph` | Lane layout: commits → rows of lanes/edges (incremental, stable across pages). Pure. | model |
| `ggt-actions` | `crates/actions` | Actions as data (argv, confirmation, refresh scope) and context menus per target | model, git |
| `ggt-ui` | `crates/ui` | ratatui widgets (graph, badges, details, menus, dialogs) and the per-frame hit-map | model, git (output types), graph, actions |
| `git-graph-tui` | `crates/app` | The `ggt` binary: CLI, config, event loop, worker-thread runner, input → events | all of the above |
| `ggt-testkit` | `crates/testkit` | Throwaway fixture repos for tests (dev-dependency only) | — |

```
Cargo.toml              # workspace: members, shared package metadata, dependency catalog, lints, profiles
rust-toolchain.toml     # pinned toolchain (rustfmt, clippy)
crates/<crate>/         # one package per crate (see table)
integrations/herdr/     # herdr plugin: opens git-graph-tui in a herdr tab
tests/fixtures/         # scripts/data for fixture repos
e2e/                    # local end-to-end tests: the real binary driven by tui-test-rs (not in the workspace or CI)
odd/tasks/              # ODD feature documents
package.json            # JavaScript workspace (Bun + Turborepo): apps/* and packages/*
apps/docs/              # documentation site (Astro Starlight), screenshots generated by apps/docs/scripts/screenshots.sh
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
