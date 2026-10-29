# Source Control "..." menu parity with VS Code

Objective: the repository "..." menu has every section of VS Code's Source Control "..." menu.
Branch: feat/scm-menu-parity, based on fix/badge-white-text (PR #43); its PR opens against main after #43 is merged (no stacked PRs). Delivery: ask-on-risk. Tests: `cargo t` (nextest).

## Specs

- **S1** "did you make sure this is same" (screenshots of VS Code's Source Control "..." menu) and "add misisng sections". Top level, in VS Code's order: View as Tree, View & Sort ›, Pull, Push, Clone, Checkout to..., Fetch, Commit ›, Changes ›, Pull, Push ›, Branch ›, Remote ›, Stash ›, Tags ›, Show Git Output. The app's own entries (Show Reflog, Worktrees...) stay after Show Git Output.
- **S2** Commit ›: Commit, Commit Staged, Commit All, Undo Last Commit, Abort Rebase (disabled unless a rebase is in progress); Commit (Amend), Commit Staged (Amend), Commit All (Amend); Commit (Signed Off), Commit Staged (Signed Off), Commit All (Signed Off). The message comes from the Source Control message box.
- **S3** Changes ›: Stage All Changes, Unstage All Changes, Discard All Changes (asks first).
- **S4** Pull, Push ›: Sync; Pull, Pull (Rebase), Pull from...; Push, Push to...; Fetch, Fetch (Prune), Fetch From All Remotes.
- **S5** Branch ›: Merge..., Rebase Branch...; Create Branch..., Create Branch From...; Rename Branch..., Delete Branch..., Delete Remote Branch...; Publish Branch....
- **S6** Remote ›: Add Remote..., Remove Remote (existing Fetch and Prune entries stay).
- **S7** Stash ›: Stash, Stash (Include Untracked), Stash Staged; Apply Latest Stash, Apply Stash...; Pop Latest Stash, Pop Stash...; Drop Stash..., Drop All Stashes...; View Stash....
- **S8** Tags ›: Create Tag..., Delete Tag..., Delete Remote Tag...; Push Tags.
- **S9** View as Tree and View & Sort ›: View as List / View as Tree, Sort Changes by Name / Path / Status (checked entry marked) for the Source Control file list.
- **S10** Clone: asks for a URL and a parent folder, runs `git clone`, and reports where it went (the app keeps showing the current repository). Assumption: no switching repositories in place.
- **S11** README: the "Staging and committing" non-goal is out of date (the Source Control pane stages and commits); update it and document the menu.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S1, S2, S3 | inline | done | feat(menu): Commit and Changes submenus |
| T2 | S4 | inline | done | feat(menu): Pull, Push submenu |
| T3 | S5, S6 | inline | done | feat(menu): Branch and Remote submenus |
| T4 | S7, S8 | inline | done | feat(menu): Stash and Tags submenus |
| T5 | S9 | inline | done | feat(menu): View as Tree and View & Sort |
| T6 | S10, S11 | inline | todo | |

Route evidence: one feature, units depend on the shared menu.rs, so no parallel-writer reason; parent works inline.

## Log

- **L1** (2026-10-29) "did you make sure this is same [Image #51] ... [Image #58]" (VS Code Source Control "..." menu: top level, Commit, Changes, Pull Push, Branch, Remote, Stash, Tags, View & Sort).
- **L2** (2026-10-29) "add misisng sections". Gap found in crates/actions/src/menu.rs `repository_menu`: only Pull..., Push..., Checkout to..., Fetch, Branch/Remote/Stash/Tags (partial) exist; Commit and Changes were README non-goals.
- **L3** (2026-10-29) T1 done: Commit and Changes submenus; menu Commit entries emit `UiEffect::Commit` and the app commits with the pane's message (shared with the pane's ✓). Gate green, 767 tests. Top-level Pull/Push/Clone/View entries are still pending (T2, T5, T6).
- **L4** (2026-10-29) "can you plan create a plugin for herdr I have notcied right click do nto work when I use on a cli on herdr plan thsi after you fnishe or ask @w2". Delegated planning to w2. Finding: herdr intercepts plain right-click for its own pane menu; `[ui] right_click_passthrough_modifier = "ctrl"` lets Ctrl+right-click through (herdr 0.9.5 config reference). Plan: integrations/herdr/ plugin (manifest, open.sh, run.sh, README), plus an in-app hint when `HERDR_ENV=1`. Not started; needs its own feature document.
- **L5** (2026-10-29) T2 done: Pull, Push submenu (Sync, Pull, Pull (Rebase), Pull from..., Push, Push to..., Fetch, Fetch (Prune), Fetch From All Remotes). Top-level Pull/Push/Fetch now run without a dialog and follow the git config, as in VS Code; the old "Pull..." mode dialog (`PullMode`, ff-only default) was removed: modern git already refuses a diverged pull unless `pull.rebase`/`pull.ff` is set. Gate green, 769 tests.
- **L6** (2026-10-29) w2 correction: plain right-click works in herdr when the pane's routing is set with `herdr pane input --current --right-click pane` (as TTT's herdr plugin does); the config modifier is only a fallback. User: "if needed ask for help to @w2 so it work on herdr plugin" → herdr plugin delegated to w2 (own worktree, own feature doc).
- **L7** (2026-10-29) T3 done: Branch submenu (Merge..., Rebase Branch..., Create Branch... now checks out, Create Branch From..., Rename Branch..., Delete Branch..., Delete Remote Branch..., Publish Branch...) via four new picker purposes; Remote: "Remove Remote". Correction to my PR #43 report: the remote branch context menu lacked "Delete Remote Branch..." and "Pull into current branch..." (VS Code has both, image 47); added here. Gate green, 775 tests.
- **L8** (2026-10-29) T4 done: Stash submenu (Stash, Include Untracked, Stash Staged; Apply/Pop latest or picked; Drop picked or all; View Stash shows `git stash show --patch`) and Tags (Delete Tag..., Delete Remote Tag..., Push Tags). The app now loads the stash list on every refresh (`Request::Stashes`). Gate green, 781 tests.
- **L9** (2026-10-29) w2 delivered the herdr plugin on feat/herdr-plugin (worktree ~/Documents/Github/Internal/tui-git-graph-herdr, commits b7b98ef, b14031c; not pushed): app routes plain right-click to itself inside herdr and restores it on exit (`TUI_GIT_GRAPH_HERDR_RIGHT_CLICK=off` disables), plugin opens tui-git-graph in a herdr tab. Plain right-click not yet tried with a real mouse.
- **L10** (2026-10-29) T5 done: View as Tree / List and View & Sort (Name, Path, Status) for the Source Control pane; folder rows in tree view stage, unstage or discard everything under them. Sort by Path keeps git's order (git status already lists by path). Not persisted across runs. Gate green, 786 tests.
