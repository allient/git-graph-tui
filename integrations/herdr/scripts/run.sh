#!/bin/sh
# Pane entrypoint: runs tui-git-graph in $TGG_TARGET_DIR.
set -u

# herdr sends plain right-click to its own pane menu by default, which hides
# tui-git-graph's context menus. The routing belongs to the pane, so set it
# before starting the app. Best effort: the app works without it.
if [ -n "${HERDR_PLUGIN_ENTRYPOINT_ID:-}" ]; then
  "${HERDR_BIN_PATH:-herdr}" pane input --current --right-click pane >/dev/null 2>&1 || true
fi

target=${TGG_TARGET_DIR:-.}
if [ -d "$target" ]; then
  cd "$target" || exit 1
fi

# $TGG_BIN, else tui-git-graph on PATH, else cargo's install directory (the
# herdr server's PATH may not include it).
bin=${TGG_BIN:-tui-git-graph}
if ! command -v "$bin" >/dev/null 2>&1 && [ -z "${TGG_BIN:-}" ] \
  && [ -x "${CARGO_HOME:-$HOME/.cargo}/bin/tui-git-graph" ]; then
  bin="${CARGO_HOME:-$HOME/.cargo}/bin/tui-git-graph"
fi
if ! command -v "$bin" >/dev/null 2>&1; then
  echo "tui-git-graph is not installed (looked for: $bin)."
  echo "Install it from a checkout of https://github.com/allient/tui-git-graph:"
  echo "  cargo install --path crates/app"
  echo
  echo "Press Enter to close."
  read -r _ || true
  exit 1
fi

# Not exec: when the app fails (not a repository, say), keep its message on
# screen instead of closing the tab at once.
"$bin" .
status=$?
if [ "$status" -ne 0 ]; then
  echo
  echo "Press Enter to close."
  read -r _ || true
fi
exit "$status"
