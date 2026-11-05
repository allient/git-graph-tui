#!/bin/sh
# Pane entrypoint: runs git-graph-tui (`ggt`) in $TGG_TARGET_DIR.
set -u

# herdr sends plain right-click to its own pane menu by default, which hides
# git-graph-tui's context menus. The routing belongs to the pane, so set it
# before starting the app. Best effort: the app works without it.
if [ -n "${HERDR_PLUGIN_ENTRYPOINT_ID:-}" ]; then
  "${HERDR_BIN_PATH:-herdr}" pane input --current --right-click pane >/dev/null 2>&1 || true
fi

target=${TGG_TARGET_DIR:-.}
if [ -d "$target" ]; then
  cd "$target" || exit 1
fi

# $TGG_BIN, else the first of `ggt` (the command since 0.3.0) and
# `git-graph-tui` (older releases) on PATH, then in cargo's install directory,
# then in Homebrew's (a herdr server started outside a login shell may have
# neither on its PATH).
bin=${TGG_BIN:-}
if [ -z "$bin" ]; then
  cargo_bin="${CARGO_HOME:-$HOME/.cargo}/bin"
  set -- ggt git-graph-tui "$cargo_bin/ggt" "$cargo_bin/git-graph-tui"
  for prefix in "${HOMEBREW_PREFIX:-}" /opt/homebrew /usr/local /home/linuxbrew/.linuxbrew; do
    [ -n "$prefix" ] && set -- "$@" "$prefix/bin/ggt" "$prefix/bin/git-graph-tui"
  done
  for candidate in "$@"; do
    if command -v "$candidate" >/dev/null 2>&1; then
      bin=$candidate
      break
    fi
  done
  bin=${bin:-ggt}
fi
if ! command -v "$bin" >/dev/null 2>&1; then
  echo "git-graph-tui is not installed (looked for: $bin)."
  echo "Install it with Homebrew:"
  echo "  brew install allient/tap/git-graph-tui"
  echo "or from a checkout of https://github.com/allient/git-graph-tui:"
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
