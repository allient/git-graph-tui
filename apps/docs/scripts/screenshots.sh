#!/bin/sh
# Regenerates the documentation screenshots in apps/docs/src/assets/screenshots.
#
# Builds git-graph-tui, creates a scratch repository with fixed dates, and
# drives the app with the tui-test CLI (https://github.com/microsoft/tui-test),
# saving one PNG per main component. Needs `tui-test` on PATH (or TUI_TEST set
# to its path), git and cargo. Local only.
set -eu

root=$(cd "$(dirname "$0")/../../.." && pwd)
out="$root/apps/docs/src/assets/screenshots"
config="$root/apps/docs/scripts/tui-test.toml"
tt=${TUI_TEST:-tui-test}
command -v "$tt" >/dev/null 2>&1 || {
    echo "tui-test not found: install it from https://github.com/microsoft/tui-test or set TUI_TEST" >&2
    exit 1
}

cargo build --quiet --manifest-path "$root/Cargo.toml" -p git-graph-tui
bin="$root/target/debug/ggt"

work=$(mktemp -d)
export TUI_TEST_SESSION="ggt-docs-$$"
cleanup() {
    "$tt" close >/dev/null 2>&1 || true
    "$tt" daemon stop --session "$TUI_TEST_SESSION" >/dev/null 2>&1 || true
    rm -rf "$work"
}
trap cleanup EXIT

# The fixture: a small project with a remote, two feature branches (one
# merged), release tags, a stash and uncommitted changes. Dates are fixed so
# the screenshots only change when the app does.
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
repo="$work/weather-cli"
git init -q --bare "$work/origin.git"
git init -q -b main "$repo"
cd "$repo"
git config user.name "Sam Rivera"
git config user.email "sam@example.com"
day=0
commit() { # commit FILE "subject" [author]
    day=$((day + 1))
    stamp="2026-10-$(printf %02d "$day")T10:00:00"
    mkdir -p "$(dirname "$1")"
    printf '%s\n' "$2" >>"$1"
    git add "$1"
    GIT_AUTHOR_DATE="$stamp" GIT_COMMITTER_DATE="$stamp" \
        git -c user.name="${3:-Sam Rivera}" commit -q -m "$2"
}
commit README.md "Initial commit"
commit src/main.rs "Add the forecast command"
commit src/api.rs "Fetch forecasts from the weather API" "Alex Kim"
git tag -a v0.1.0 -m "v0.1.0"
git remote add origin "$work/origin.git"
git push -q -u origin main --tags
git switch -q -c feature/units
commit src/units.rs "Support metric and imperial units" "Alex Kim"
commit src/units.rs "Remember the chosen units" "Alex Kim"
git switch -q main
commit src/api.rs "Retry failed API requests"
GIT_AUTHOR_DATE="2026-10-07T10:00:00" GIT_COMMITTER_DATE="2026-10-07T10:00:00" \
    git merge -q --no-ff feature/units -m "Merge branch 'feature/units'"
commit CHANGELOG.md "Release v0.2.0"
git tag -a v0.2.0 -m "v0.2.0"
git push -q origin main --tags
git switch -q -c fix/timeout
commit src/api.rs "Time out slow requests after 10 seconds"
git push -q -u origin fix/timeout
git switch -q main
commit src/main.rs "Show the weekly forecast"
printf 'draft\n' >>src/main.rs
GIT_AUTHOR_DATE="2026-10-10T10:00:00" GIT_COMMITTER_DATE="2026-10-10T10:00:00" \
    git stash push -q -m "Try a compact layout"
printf '// colors\n' >>src/main.rs
printf 'notes\n' >NOTES.md

home="$work/home"
mkdir -p "$home"
"$tt" run --config "$config" --cols 140 --rows 30 --cwd "$repo" \
    --env HOME="$home" --env XDG_CONFIG_HOME="$home/.config" \
    --env GIT_CONFIG_GLOBAL=/dev/null --env GIT_CONFIG_NOSYSTEM=1 \
    -- sh -c 'printf "\033]0;git-graph-tui\007"; exec "$0" "$@"' "$bin" --icons unicode "$repo" >/dev/null
"$tt" expect text "Show the weekly forecast" --timeout 10000 >/dev/null

mkdir -p "$out"
shot() { # shot NAME: wait for drawing to settle, then save NAME.png
    sleep 0.4
    "$tt" screenshot -o "$out/$1.png" >/dev/null
    echo "saved $out/$1.png"
}
key() { "$tt" key press "$@" >/dev/null; }
click() { "$tt" click text "$@" >/dev/null; }
expect() { "$tt" expect text "$1" --timeout 5000 >/dev/null; }

shot graph

# The badge, not the merge commit's subject above it.
click "feature/units" --match last --button right
expect "Checkout Branch"
shot branch-menu
key Escape

click "Retry failed API requests" --button right
expect "Cherry Pick"
shot commit-menu
key Escape

click "Fetch forecasts from the weather API"
key Enter
expect "Changed files"
shot commit-details
key Escape

key /
"$tt" type "forecast" >/dev/null
expect "of"
shot find
key Escape

click "…"
expect "Pull"
shot repository-menu
key Escape

click "Show the weekly forecast" --button right
click "Create Branch..."
expect "Branch name"
"$tt" type "feature/alerts" >/dev/null
shot create-branch
key Escape

key q
