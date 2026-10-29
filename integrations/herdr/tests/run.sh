#!/bin/sh
# Tests for the herdr plugin scripts, with herdr and tui-git-graph stubbed.
# Run from anywhere: sh integrations/herdr/tests/run.sh
set -u
SH=${SH:-sh}

here=$(cd "$(dirname "$0")" && pwd)
scripts="$here/../scripts"
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
failures=0

# A herdr stub that records one line of argv per call, exiting with $STUB_EXIT.
mkdir -p "$scratch/bin"
cat > "$scratch/bin/herdr" <<'STUB'
#!/bin/sh
printf '%s\n' "$*" >> "$STUB_LOG"
exit "${STUB_EXIT:-0}"
STUB
# A tui-git-graph stub that records where it ran and with what.
cat > "$scratch/bin/tui-git-graph" <<'STUB'
#!/bin/sh
printf 'cwd=%s args=%s\n' "$(pwd)" "$*" >> "$STUB_LOG"
exit "${STUB_APP_EXIT:-0}"
STUB
chmod +x "$scratch/bin/herdr" "$scratch/bin/tui-git-graph"
export STUB_LOG="$scratch/log"
mkdir -p "$scratch/repo"
repo=$(cd "$scratch/repo" && pwd -P)

check() { # name expected actual
  if [ "$2" = "$3" ]; then
    echo "ok   $1"
  else
    echo "FAIL $1"
    echo "  expected: $2"
    echo "  actual:   $3"
    failures=$((failures + 1))
  fi
}

reset() { : > "$STUB_LOG"; }

open_with() { # context-json
  reset
  HERDR_BIN_PATH="$scratch/bin/herdr" HERDR_PLUGIN_CONTEXT_JSON="$1" \
    $SH "$scripts/open.sh" </dev/null
  cat "$STUB_LOG"
}

opens="plugin pane open --plugin tui-git-graph --entrypoint graph --focus --env TGG_TARGET_DIR"

check "open: checkout_path wins" "$opens=/wt" \
  "$(open_with '{"workspace_cwd":"/ws","focused_pane_cwd":"/pane","checkout_path":"/wt"}')"
check "open: then focused_pane_cwd" "$opens=/pane" \
  "$(open_with '{"workspace_cwd":"/ws","focused_pane_cwd":"/pane"}')"
check "open: then workspace_cwd" "$opens=/ws" \
  "$(open_with '{"workspace_cwd":"/ws"}')"
check "open: then ." "$opens=." "$(open_with '{}')"
check "open: no context at all" "$opens=." "$(open_with '')"
check "open: paths with spaces" "$opens=/my repo/x" \
  "$(open_with '{"checkout_path": "/my repo/x"}')"

run_with() { # entrypoint-id herdr-exit [binary]
  reset
  HERDR_BIN_PATH="$scratch/bin/herdr" HERDR_PLUGIN_ENTRYPOINT_ID="$1" STUB_EXIT="$2" \
    TGG_TARGET_DIR="$repo" TGG_BIN="${3:-$scratch/bin/tui-git-graph}" \
    $SH "$scripts/run.sh" </dev/null >"$scratch/out" 2>&1
  echo "exit=$?"
  cat "$STUB_LOG"
}

nl='
'
check "run: routes right-click, then starts the app in the target" \
  "exit=0${nl}pane input --current --right-click pane${nl}cwd=$repo args=." \
  "$(run_with graph 0)"
check "run: a failing herdr doesn't stop the app" \
  "exit=0${nl}pane input --current --right-click pane${nl}cwd=$repo args=." \
  "$(run_with graph 1)"
check "run: outside a plugin pane, no routing" \
  "exit=0${nl}cwd=$repo args=." "$(run_with '' 0)"
check "run: the app's failure is kept on screen and passed on" "exit=3 Press Enter to close." \
  "$(STUB_APP_EXIT=3 run_with graph 0 | head -1) $(tail -1 "$scratch/out")"
check "run: a missing binary exits 1" "exit=1" \
  "$(run_with '' 0 "$scratch/missing/tui-git-graph" | head -1)"
check "run: a missing binary shows the install hint" "yes" \
  "$(grep -q 'cargo install' "$scratch/out" && echo yes || echo no)"

if [ "$failures" -ne 0 ]; then
  echo "$failures failed"
  exit 1
fi
echo "all passed"
