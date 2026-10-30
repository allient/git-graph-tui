#!/bin/sh
# Builds git-graph-tui, then drives it in a terminal with tui-test-rs.
# Local only: needs Rust 1.90+ (the pinned toolchain), git, and a Unix pty.
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
cargo build --manifest-path "$root/Cargo.toml" -p git-graph-tui
GIT_GRAPH_TUI_BIN="$root/target/debug/ggt" \
    cargo test --manifest-path "$root/e2e/Cargo.toml" "$@"
