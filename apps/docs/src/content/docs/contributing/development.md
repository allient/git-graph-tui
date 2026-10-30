---
title: Development
description: The monorepo layout, how to build and test the app, and how to work on these docs.
---

## The monorepo

The repository holds the Rust app and this documentation site side by side:

```
Cargo.toml          # Rust workspace: the app and its crates
crates/<crate>/     # one package per crate (see below)
integrations/herdr/ # herdr plugin
e2e/                # local end-to-end tests (tui-test-rs)
package.json        # JavaScript workspace (Bun): apps/* and packages/*
turbo.json          # Turborepo tasks
apps/docs/          # this site (Astro Starlight)
packages/           # shared JavaScript packages (none yet)
```

Cargo only sees the Rust crates, and Bun and Turborepo only see `apps/*` and `packages/*`, so neither side gets in the other's way.

## The Rust app

You need Rust (the toolchain is pinned in `rust-toolchain.toml`) and `git`.

```sh
cargo run-app            # run the TUI (alias for cargo run -p git-graph-tui --)
cargo t                  # run all tests (cargo nextest)
cargo lint               # clippy on every crate, warnings are errors
cargo fmt --all          # format
cargo deny check         # licenses, advisories, sources
cargo machete            # unused dependencies
```

Tools: `cargo install cargo-nextest cargo-insta cargo-deny cargo-machete`.

### Crates

| Crate | Path | Role |
| --- | --- | --- |
| `ggt-model` | `crates/model` | Domain types: commits, refs, signatures |
| `ggt-git` | `crates/git` | Safe git argv building, process execution, output parsers |
| `ggt-graph` | `crates/graph` | Lane layout: commits to rows of lanes and edges |
| `ggt-actions` | `crates/actions` | Actions as data and the context menus per target |
| `ggt-ui` | `crates/ui` | ratatui widgets and the per-frame hit-map |
| `git-graph-tui` | `crates/app` | The binary: CLI, config, event loop, git runner |
| `ggt-testkit` | `crates/testkit` | Throwaway fixture repositories for tests |

Data flows one way: input becomes an event, `update(state, event)` changes the state and returns commands, git runs on a worker thread, and `render(state)` draws. Each frame records a hit-map of what was drawn where, so clicks resolve exactly.

### End-to-end tests

`sh e2e/run.sh` builds the binary and drives it in a real terminal with [tui-test-rs](https://github.com/microsoft/tui-test): real keys and mouse clicks, checking the screen and the repository's git state. They run locally, not in CI.

## These docs

You need [Bun](https://bun.sh/).

```sh
bun install              # once
bun run dev:docs         # http://localhost:4321, reloads as you edit
bun run build:docs       # static site in apps/docs/dist
bun run preview:docs     # serve the built site
```

Pages are Markdown files in `apps/docs/src/content/docs/`. Add a new page to the `sidebar` in `apps/docs/astro.config.mjs`.

### Screenshots

The screenshots in `apps/docs/src/assets/screenshots/` are generated, not taken by hand. The script builds the app, creates a sample repository with fixed dates, and drives the app with the [tui-test CLI](https://github.com/microsoft/tui-test):

```sh
bun run screenshots      # or: sh apps/docs/scripts/screenshots.sh
```

It needs `tui-test` on your `PATH` (`brew tap microsoft/tui-test https://github.com/microsoft/tui-test && brew install tui-test`), or `TUI_TEST` set to its path. Rerun it after a UI change, and commit the new images.

## Releasing

Set the version in the root `Cargo.toml`, merge it, then push a tag such as `v0.1.1`. The release workflow builds every platform, publishes the GitHub Release with archives, checksums and installers, and updates the Homebrew formula.
