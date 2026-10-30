#!/bin/sh
# Tests for the herdr plugin scripts, with herdr and the app (`ggt`) stubbed.
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
# git-graph-tui stubs that record where they ran and with what. `ggt` is the
# command since 0.3.0, `git-graph-tui` the one before; each lives in its own
# folder so a test can put either, or both, on PATH.
stub_app() { # dir name
  mkdir -p "$1"
  cat > "$1/$2" <<'STUB'
#!/bin/sh
printf '%s cwd=%s args=%s\n' "$(basename "$0")" "$(pwd)" "$*" >> "$STUB_LOG"
exit "${STUB_APP_EXIT:-0}"
STUB
  chmod +x "$1/$2"
}
stub_app "$scratch/bin" ggt
stub_app "$scratch/old" git-graph-tui
chmod +x "$scratch/bin/herdr"
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

opens="plugin pane open --plugin git-graph-tui --entrypoint graph --focus --env TGG_TARGET_DIR"

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
    TGG_TARGET_DIR="$repo" TGG_BIN="${3:-$scratch/bin/ggt}" \
    $SH "$scripts/run.sh" </dev/null >"$scratch/out" 2>&1
  echo "exit=$?"
  cat "$STUB_LOG"
}

nl='
'
check "run: routes right-click, then starts the app in the target" \
  "exit=0${nl}pane input --current --right-click pane${nl}ggt cwd=$repo args=." \
  "$(run_with graph 0)"
check "run: a failing herdr doesn't stop the app" \
  "exit=0${nl}pane input --current --right-click pane${nl}ggt cwd=$repo args=." \
  "$(run_with graph 1)"
check "run: outside a plugin pane, no routing" \
  "exit=0${nl}ggt cwd=$repo args=." "$(run_with '' 0)"
check "run: the app's failure is kept on screen and passed on" "exit=3 Press Enter to close." \
  "$(STUB_APP_EXIT=3 run_with graph 0 | head -1) $(tail -1 "$scratch/out")"
check "run: a missing binary exits 1" "exit=1" \
  "$(run_with '' 0 "$scratch/missing/ggt" | head -1)"
check "run: a missing binary shows the install hint" "yes" \
  "$(grep -q 'brew install allient/tap/git-graph-tui' "$scratch/out" && echo yes || echo no)"

run_on_path() { # extra-PATH-dirs
  reset
  PATH="$1:/usr/bin:/bin" CARGO_HOME="$scratch/no-cargo" TGG_BIN= \
    TGG_TARGET_DIR="$repo" $SH "$scripts/run.sh" </dev/null >"$scratch/out" 2>&1
  echo "exit=$?"
  cat "$STUB_LOG"
}

check "run: finds ggt on PATH" "exit=0${nl}ggt cwd=$repo args=." \
  "$(run_on_path "$scratch/bin")"
check "run: falls back to git-graph-tui from older releases" \
  "exit=0${nl}git-graph-tui cwd=$repo args=." "$(run_on_path "$scratch/old")"
check "run: prefers ggt when both are installed" "exit=0${nl}ggt cwd=$repo args=." \
  "$(run_on_path "$scratch/old:$scratch/bin")"
mkdir -p "$scratch/cargo/bin"
stub_app "$scratch/cargo/bin" ggt
check "run: then cargo's install directory" "exit=0${nl}ggt cwd=$repo args=." \
  "$(reset; PATH=/usr/bin:/bin CARGO_HOME="$scratch/cargo" TGG_BIN= TGG_TARGET_DIR="$repo" \
    $SH "$scripts/run.sh" </dev/null >/dev/null 2>&1; echo "exit=$?"; cat "$STUB_LOG")"

if [ "$failures" -ne 0 ]; then
  echo "$failures failed"
  exit 1
fi
echo "all passed"
