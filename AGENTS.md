# AGENTS.md

Guidance for AI agents (and humans) working on **tui-git-graph**, a standalone terminal Git Graph written in Rust with ratatui. Read `README.md` first for the goals, scope, stack and architecture. This file covers **how** to build it.

> If a fact here conflicts with the code or with a library's docs, check the source, fix this file, and don't guess.

---

## 1. Ground rules

- **The only runtime dependency is the `git` CLI.** No libgit2, no editor or multiplexer APIs. herdr and TTT are integrations that launch the binary (`integrations/`). They must never become dependencies of the core or app crates.
- **Pure core, thin shell.** `crates/core` has no terminal, no threads, no I/O except running git through a trait. Everything that can be a pure function (parsing, lane layout, filtering, search, menu building, action planning) lives there and is unit-tested.
- **One source of truth.** `AppState` in `crates/app/state` is the only mutable UI state. Input becomes an `Event`, `update(state, event)` changes state and returns `Command`s, and `render(&state)` draws. No widget owns hidden state that affects behavior.
- **Never block the UI thread.** Every git call runs on a worker thread through the runner, with a timeout and cancellation. Results come back as `Event::GitResult` tagged with a generation id; stale results are dropped.
- **Ask before** adding a dependency that pulls a large tree (async runtimes, libgit2, web stacks), changing the CLI surface, or touching release/distribution config.

---

## 2. Workspace layout

```
Cargo.toml              # workspace; shared lints, MSRV, profiles
crates/core/            # lib: git argv + parsers, model, graph layout, actions
crates/app/             # bin `tui-git-graph`: runner, state, ui, input
integrations/herdr/     # herdr-plugin.toml (+ scripts)
integrations/ttt/       # plugin.ttt.json + init.lua that opens the binary in a terminal tab
tests/fixtures/         # scripts that build fixture repos in temp dirs
odd/tasks/              # ODD feature documents (specs + tasks + log)
docs/                   # tooling and design docs
```

Module rules:
- `core` never depends on `app`, ratatui or crossterm.
- `app/ui` only reads state and produces frames plus a **hit-map** (cell → target). It never runs git.
- `app/input` turns crossterm events into `Event`s using the last frame's hit-map. Mouse coordinates are terminal-absolute, and we own the layout, so hit-testing is exact.

---

## 3. Git best practices

- **Machine-readable output only.** Never parse output meant for humans.
  - Log: `git -C <repo> log --topo-order -z --format=%H%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%cn%x1f%ce%x1f%ct%x1f%D%x1f%s` (switchable to `--date-order`). Fields are split on `%x1f` and records on NUL, because subjects can contain anything.
  - Refs: `git for-each-ref --format=...` (include `%(upstream)` and `%(upstream:track)` for ahead/behind).
  - Status: `git status --porcelain=v2 -z --branch`.
  - Stashes: `git stash list -z --format=%H%x1f%gd%x1f%s`.
- **Always** pass `-C <repo>`, `-c color.ui=never` and `-c core.quotepath=off`, and use `--` before paths. Set `GIT_TERMINAL_PROMPT=0` and `GIT_OPTIONAL_LOCKS=0` for read-only calls so background refreshes never prompt or contend for locks.
- **Build argv, never shell strings.** Branch, tag and remote names are user input: pass them as separate arguments, validate with `git check-ref-format --branch`, and never interpolate into `sh -c`.
- **Paginate.** Load about 1,000 commits at a time and continue from the last loaded commit. Lane layout must be **incremental and stable**: appending a page never changes rows already drawn.
- **Map errors to messages.** Translate common stderr (not a repo, conflicts, non-fast-forward, uncommitted changes would be overwritten, auth failure) into a clear status-bar message. Keep the full command and stderr in the git log view ("Show Git Output").
- **Network operations** (fetch, pull, push) get longer timeouts, show progress and are cancellable. If credentials are needed, suspend the TUI and run the command attached to the terminal so the user can answer the prompt, then resume.
- **After any mutating action**, refresh refs, status and the loaded range, keeping the selection on the same commit hash.

---

## 4. Safety rules

- Every destructive action shows a confirmation dialog that names exactly what will happen: hard reset, branch delete (especially unmerged `-D`), force push (prefer `--force-with-lease`), `clean`, stash drop, tag delete (local and remote), drop commit, rebase, detached checkout.
- Default to the safer variant: `--force-with-lease` over `--force`, `-d` before offering `-D`, `reset --mixed` as the default reset mode.
- Detect in-progress states (merge, rebase, cherry-pick, revert, via `git rev-parse --git-path`). Show a banner and block conflicting actions.
- **Tests never touch a real repo.** Every test that mutates uses a throwaway fixture repo in a temp dir.

---

## 5. UI / UX conventions

- Match the VS Code Git Graph look and behavior described in the README and `odd/tasks/git-graph-mvp.md` (colored lanes, ref badges, muted non-HEAD-ancestors, inline commit details, context menus per target).
- **Mouse and keyboard parity:** click selects, double-click or Enter expands details, right-click or the menu key (`m` / Shift+F10) opens the context menu for the selection, the wheel scrolls, Esc closes. j/k or arrows, PgUp/PgDn, g/G, `/` to find.
- Popup menus open at the cursor (mouse) or next to the selection (keyboard), stay inside the terminal, and support arrows, Enter, Esc and accelerator letters.
- Graph glyphs use box-drawing characters (`│ ─ ╭ ╮ ╰ ╯ ┼`) and `●` / `○` for commits, with an ASCII fallback (config).
- Colors: a lane palette that works on dark and light themes, overridable in config. Respect `NO_COLOR`.
- Measure text width with `unicode-width`, never byte length. CJK and emoji must not break column alignment.
- Narrow terminals (herdr/tmux panes): hide columns progressively (commit, author, date), and keep graph and subject last.
- Performance target: first paint < 200 ms on a 10k-commit repo, smooth scrolling on 100k. Measure before optimizing.

---

## 6. Testing

- **Unit (core):** parsers fed with real captured git output (weird subjects, unicode, empty repo, octopus merges, detached HEAD, shallow clones), lane layout snapshot-tested with `insta` (ASCII rendering of lanes), filters, search, menu building and action planning (argv + confirmation text).
- **Integration (core + git):** build fixture repos with the scripts in `tests/fixtures/` (branches, merges, tags, stash, a local bare repo as `origin`) in a temp dir, run actions, and assert the resulting repo state with git.
- **UI:** render `AppState`s into ratatui's `TestBackend` and snapshot the buffer with `insta`. Drive key and mouse `Event`s through `update` and assert state, including hit-testing for right-click menus.
- Run with `cargo nextest run`. Review snapshot changes with `cargo insta review`.
- **Definition of done for a feature:** core unit tests, an integration test against a fixture repo for each mutating action, a UI snapshot for each new visual, destructive paths confirmed, the README checklist item ticked.

---

## 7. Code style

- Rust stable, 2024 edition. `cargo fmt` and `cargo clippy --all-targets -- -D warnings` must pass. `cargo deny check` for licenses and advisories.
- No `unwrap()` / `expect()` outside tests and truly impossible states (comment why). `thiserror` errors in `core`, `anyhow` with context in `app`.
- Small modules, explicit types at boundaries, no global mutable state. Prefer enums plus `match` for events, commands and actions.
- Comments explain *why* (git quirks, terminal quirks), not *what*.
- **No code copied from vscode-git-graph** (its license forbids it). Studying behavior is fine. git-graph, git-igitt and serie (MIT) and git-graph-plus (Apache-2.0) can be read for ideas; credit anything substantial that is adapted.

---

## 8. Workflow

This repo is developed with **[Gentle AI](https://github.com/Gentleman-Programming/gentle-ai)** (ODD workflow, RDD review, Engram memory) and a few skills.sh skills. Setup and details are in [`docs/ai-tooling.md`](docs/ai-tooling.md). Repo-specific ODD rules:

- Substantial work gets one feature document in `odd/tasks/<feature-name>.md`, one per README roadmap stage or large checklist item. Commit it.
- ODD's test-first step uses **`cargo nextest`** (+ `insta`) with fixture repos. Never invent another runner.
- Commit in about 400-line work units on a feature branch with Conventional Commits. Use the `chained-pr` skill for larger changes.
- Before saying a change is done, run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo nextest run`.

---

## 9. Reference

- ratatui: <https://ratatui.rs/> (concepts, mouse events, `TestBackend`)
- Graph layout references: [git-graph](https://github.com/git-bahn/git-graph) (MIT), [serie](https://github.com/lusingander/serie) (MIT, ratatui), [git-igitt](https://github.com/git-bahn/git-igitt) (MIT)
- Behavior reference: [vscode-git-graph](https://github.com/mhutchie/vscode-git-graph) (behavior only, no code)
- herdr plugin format: see `herdr-plugin/` in <https://github.com/eugenioenko/ttt> (a manifest that runs a command in a pane)
- TTT plugin API (for `integrations/ttt` only): <https://tttedit.dev/guides/plugin-authoring/>. On v1.9.0, open the binary with `ttt.terminal.spawn` (`terminal.control` permission).
