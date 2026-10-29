#!/bin/sh
# Action entrypoint: opens tui-git-graph for the active repository.
#
# Actions run headless (no terminal), so this hands over to the `graph` pane
# entrypoint, which herdr runs in a real pane. The directory travels through
# --env, not --cwd: herdr starts plugin commands from the plugin root, and
# the pane command is a relative path that --cwd would make unfindable.
set -eu

# Prints the string value of "$1" in $HERDR_PLUGIN_CONTEXT_JSON, if any.
context() {
  printf '%s' "${HERDR_PLUGIN_CONTEXT_JSON:-}" \
    | grep -o "\"$1\"[[:space:]]*:[[:space:]]*\"[^\"]*\"" \
    | head -n 1 \
    | sed -e "s/^\"$1\"[[:space:]]*:[[:space:]]*\"//" -e 's/"$//' \
    || true
}

# The active worktree, else the focused pane's directory, else the workspace's.
dir=$(context checkout_path)
[ -n "$dir" ] || dir=$(context focused_pane_cwd)
[ -n "$dir" ] || dir=$(context workspace_cwd)
[ -n "$dir" ] || dir=.

exec "${HERDR_BIN_PATH:-herdr}" plugin pane open \
  --plugin tui-git-graph \
  --entrypoint graph \
  --focus \
  --env "TGG_TARGET_DIR=$dir"
