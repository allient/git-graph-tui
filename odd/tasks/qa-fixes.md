# Release QA fixes (before v0.1.0)

Objective: fix what release-candidate QA found, so every README feature works before tagging v0.1.0.
Branch: build/release-setup (same PR as the release setup). Delivery: ask-on-risk. Tests: `cargo t` (nextest), then the pty QA scripts rerun on a new release binary.

## Specs

- **S1** "before deplot first version make sure all features are working as expected". QA ran ~195 end-to-end checks on the release binary (eb962c3); these failed:
- **S2** Stashes: README "Local and remote branches, tags, stashes and remote HEAD labels, as colored badges" and "Actions on tags and stashes: Apply, pop, drop stashes; create branch from stash". Each stash shows as its own row above the commit it was made on, with a stash badge, and right-click opens the stash menu (QA graph F1, menus F1).
- **S3** Keyboard menus: README "`m` / right click: Context menu for the commit, branch, tag, stash or uncommitted row". On a row with badges, `m` / Shift+F10 must reach the branch, tag, remote and stash menus too (QA menus F2).
- **S4** After choosing an entry of the "..." menu opened from the Source Control header, keys go to the graph again, not into the commit message box (QA scm note 2).
- **S5** In option dialogs, Left/Right must not toggle a focused checkbox (only Space does), so moving to a button can't tick "Force delete" or "Also delete it on the remote" (QA scm note 3).
- **S6** A merge, rebase, cherry-pick or revert that stops on conflicts says so ("... stopped on conflicts: resolve them, then Continue or Abort") instead of "failed: git failed" (QA menus F3).
- **S7** A detached HEAD's commit is marked (a `HEAD` badge), and an empty repository says "No commits yet" (QA graph F3, F4).
- **S8** The stash picker's filter matches the stash message too, not only `stash@{n}` (QA scm note 4).
- **S9** Create Tag with Type "Annotated" and an empty message creates an annotated tag (QA scm note 5; contradicts QA menus, so reproduce first).

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S2 | inline | done | fix(graph): stash rows |
| T2 | S3 | inline | done | fix(app): keyboard badge menus |
| T3 | S4, S5 | inline | done | fix(ui): focus and checkboxes |
| T4 | S6, S7, S8, S9 | inline | todo | |
| T5 | S1 | QA rerun on a new release binary | todo | |

Route evidence: the fixes share crates/app/src/state.rs, so they run inline one after another.

## Log

- **L1** (2026-10-29) "before deplot first version make sure all features are working as expected and use argent or any other mcp to test all features work as extected". QA results: graph/views mostly PASS (F1 stash rows, F2 README line fixed in 5ba17fb, F3 detached marker, F4 empty repo); context menus 66/69 (stash menu unreachable, keyboard branch menus, conflict message); Source Control + "..." menu 60/60 with five notes.
- **L2** Not fixed now (by design or later): no auto-refresh on outside file changes (press r); Cherry Pick/Revert run without a dialog; non-destructive confirmations focus the confirm button.
- **L3** (2026-10-29) T1 done: `git stash list` is read in the log format (`stash::parse_commits`, first parent only); the app inserts each stash just above its base commit as history pages load (`log_loaded` counts real commits for paging) and re-lays out when the stash list arrives after the history; one `stash@{n}` badge per stash (`visible_refs`), and right-clicking a stash row or badge opens the stash menu. Tests that counted the sample repo's 7 rows now count its stash row (8). 802 tests pass.
- **L4** (2026-10-29) T2 done: `m` / Shift+F10 on a commit row with badges opens one submenu per branch, remote branch (including remote names on a local badge) and tag (`Branch main ▸`, `Remote origin/main ▸`, `Tag v0.1.0 ▸`), then the commit's entries; a stash row keeps its stash menu. 803 tests pass.
- **L5** (2026-10-29) T3 done: opening the "..." menu from the Source Control header gives the keys back to the graph; dialog checkboxes toggle only with Space (arrows still cycle choices). RED observed on the old code for the checkbox test. 805 tests pass.
