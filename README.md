# tui-git-graph

> A Git Graph for the terminal. See your repository's colored commit graph and act on it with right-click menus (checkout, branch, merge, rebase, cherry-pick, reset, push, and more), just like the Git Graph extension for VS Code. A single native binary that works in any terminal, inside [herdr](https://herdr.dev/) panes and inside the [TTT editor](https://tttedit.dev/).

## Concept

Git Graph for VS Code is one of the most loved Git tools because it combines two things in one view:

1. **See** the whole repository at a glance: every local and remote branch, tag, stash, merge and the uncommitted changes, drawn as a clear, color-coded tree.
2. **Act** on it directly: right-click a commit, branch or tag to checkout, merge, rebase, cherry-pick, reset, push, and more.

In the terminal, this combination doesn't exist today. Existing tools either draw a great graph but are read-only ([serie](https://github.com/lusingander/serie), [git-graph](https://github.com/git-bahn/git-graph), [git-igitt](https://github.com/git-bahn/git-igitt)), or manage Git well but with a weaker graph (lazygit, tig). Terminal-first editors and agent workspaces (TTT, Orca, herdr) have the same gap: users keep asking for a "real" Git Graph view.

**tui-git-graph** fills that gap as a standalone terminal app with full mouse support, so it can run anywhere a terminal runs.

## Goals

**Main goal:** offer the capabilities of the Git Graph extension for VS Code, adapted to a terminal UI.

- **Independent.** A single binary whose only runtime dependency is the `git` CLI. No editor, multiplexer or plugin API required.
- **Mouse and keyboard.** Click to select, right-click for context menus, scroll with the wheel. Everything also works from the keyboard. Inside herdr, tui-git-graph routes plain right-click to itself while it runs (`herdr pane input --right-click pane`) and gives it back on exit; set `TUI_GIT_GRAPH_HERDR_RIGHT_CLICK=off` to keep herdr's menu.
- **Fast** on real repositories (tens of thousands of commits, many branches): incremental loading, background git calls, a UI that never blocks.
- **Safe.** Every destructive action (reset, delete branch, force push, drop, clean) asks for confirmation and says exactly what will happen.
- **Works where you work.** Any terminal, tmux, herdr (a [herdr plugin](integrations/herdr/README.md) opens it in a tab; plain right-click works inside herdr) and TTT (via a thin TTT plugin that opens it in a terminal tab).
- **Maintainable.** A pure, well-tested core (parsing, layout, actions) with a thin UI layer.

### Non-goals (for now)

- A full editor for commits: the Source Control pane stages, discards and commits (also amend and sign-off) with a short message; for long messages or partial staging (hunks), use your editor or `git`.
- A text editor or a general IDE.
- Version control systems other than Git.
- Reusing code from the Git Graph extension. Everything is written from scratch (see [Licensing](#licensing)).

## Scope: feature parity checklist

Target feature set, based on Git Graph for VS Code. The detailed specs for the first release are in [`odd/tasks/git-graph-mvp.md`](odd/tasks/git-graph-mvp.md).

### Graph view
- [x] Commit graph with color-coded lanes per branch and merge lines, drawn with thick lanes (`┃`) and rounded S-curves where a line changes lanes (`●╮`, `┃╰╮`) in Git Graph's own palette (blue, magenta, green, orange, purple, red, cyan, ...)
- [x] Local and remote branches, tags, stashes and remote HEAD labels, as colored badges
- [x] Uncommitted changes shown as a virtual row at the top
- [x] Columns: graph, description, date, author, commit hash (show/hide, adapt to width)
- [x] Highlight HEAD and mute commits that aren't ancestors of HEAD
- [x] Incremental loading while scrolling
- [x] Branch filter: show all, one or several branches, or custom glob patterns
- [x] Toggle remote branches on/off
- [x] Double-click a branch, or a commit a branch points at, to check it out
- [x] Resize the panes and columns by dragging their dividers (remembered in the config file)
- [x] Find: search by message, author, date, hash, branch or tag name
- [x] Jump to HEAD, refresh, and fetch from the toolbar

### Source Control pane (left of the graph)
- [x] Staged Changes and Changes sections with status letters (M, A, D, R, C, T, U, !)
- [x] Stage, unstage and discard a file or a whole section, by button (`+` `−` `↶`) or key
- [x] Open a file's changes (working tree or staged; an untracked file shows every line added), or "Open Changes" (`⧉`) for every file of a section in one view
- [x] "Open File" (`↗`) in your editor (`$VISUAL`, `$EDITOR`, else `vi`)
- [x] Multi-line commit message and a Commit button; commit, refresh and "..." buttons in the header
- [x] Resizable by dragging the divider; hidden on terminals under 90 columns

### Actions on branches (right-click a branch badge)
- [x] Checkout (local, or create a tracking branch from a remote one), create, rename, delete
- [x] Merge into current branch, rebase current branch onto it, reset
- [x] Pull, push, fetch into local branch
- [x] Create archive, copy branch name
- [x] Create a pull request (GitHub / GitLab / Bitbucket, opens the browser)

### Actions on commits (right-click a commit)
- [x] Add tag, create branch
- [x] Checkout (detached), cherry-pick, revert, drop
- [x] Merge into current branch, rebase current branch on this commit
- [x] Interactive rebase from here (reorder, squash, fixup, drop the commits after it)
- [x] Reset current branch to this commit (soft / mixed / hard)
- [x] Copy hash / subject to clipboard

### Repository menu ("..." in the Source Control header, or the toolbar when the pane is hidden)
- [x] Pull, push, fetch, "Checkout to..." (pick any branch or tag). Pull, Push and Fetch run right away and follow your git config (`pull.rebase`, `pull.ff`, `push.default`); a diverged pull stops unless git is configured to merge or rebase
- [x] Clone: asks for a repository URL and a parent folder (the one holding this repository by default), clones into a folder named after the repository and says where it went; open that one with `tui-git-graph <folder>`
- [x] Pull, Push submenu, as in VS Code: Sync (pull, then push); Pull, Pull (Rebase), Pull from... (a remote and branch); Push, Push to... (remote, set upstream, force with lease); Fetch, Fetch (Prune), Fetch From All Remotes
- [x] Branch, remote, stash and tag submenus; show the git command log
- [x] Stash submenu, as in VS Code: Stash, Stash (Include Untracked), Stash Staged (git 2.35 or newer); Apply Latest Stash, Apply Stash...; Pop Latest Stash, Pop Stash...; Drop Stash..., Drop All Stashes... (asks first); View Stash... (its diff). The "..." entries pick a stash from a list
- [x] Tags submenu: Create Tag..., Delete Tag..., Delete Remote Tag... (keeps the local tag), Push Tags
- [x] Branch submenu, as in VS Code: Merge... and Rebase Branch... (pick a branch), Create Branch... (from HEAD, checked out), Create Branch From... (pick a branch or tag), Rename Branch..., Delete Branch..., Delete Remote Branch... (asks first), Publish Branch... (push with upstream); Remote: Add Remote..., Remove Remote, Fetch, Prune
- [x] View as Tree / View as List and View & Sort, as in VS Code: the Source Control files flat (name, then folder) or grouped under their folders, whose row buttons stage, unstage or discard everything under them; sorted by path (git's order), name or status; `✓` marks the current choices, and the choice is saved for next time
- [x] Commit submenu, as in VS Code's Source Control: Commit (what is staged, or everything when nothing is), Commit Staged, Commit All (untracked files too), each also as Amend (an empty message keeps the last one) and Signed Off; Undo Last Commit (its changes stay staged); Abort Rebase while a rebase is stopped. The message is the one typed in the Source Control pane
- [x] Changes submenu: Stage All, Unstage All and Discard All Changes (asks first; untracked files are deleted, staged changes kept)
- [x] Show the reflog and recover commits from it
- [x] Worktrees: list, add, remove, lock, unlock and prune them

### Actions on tags and stashes
- [x] Add, delete and push tags; view annotated tag details
- [x] Apply, pop, drop stashes; create branch from stash

### Actions on uncommitted changes
- [x] Stash, reset, clean

### Commit details
- [x] Click a commit to expand its metadata (hash, parents, author, committer, date) and changed files: bold labels, parent hashes and email addresses as links, and a folder tree whose file names are colored by change (added green, modified yellow, deleted red, renamed blue), with `(+added | -deleted)` counts in green and red (new files show none)
- [x] The details open below the commit like in VS Code: the lanes keep running through them, the text starts at the Description column, and the message and the file list scroll on their own (mouse wheel, or `J`/`K` for the message)
- [x] Buttons at the details' right edge: close, find (filter the files by name), tree view and list view (full paths)
- [x] View the diff of any file (built-in viewer, or your configured `git difftool` / pager such as delta)
- [x] View a file at that revision
- [x] Compare any two commits, or a commit with the working tree

### Repository settings
- [x] Manage remotes (add, remove, fetch, prune)
- [x] Issue linking (turn `#123` into links that open in the browser)

### Stretch goals (inspired by Git Graph+)
- [x] Interactive rebase UI (reorder, squash, fixup, drop)
- [x] Conflict detection banner with continue / abort
- [x] Worktree management
- [x] Reflog view with recovery of lost commits

## Tech stack

| Area | Choice | Why |
| --- | --- | --- |
| Language | **Rust** (stable, 2024 edition) | A single fast binary. The best reference code for graph layout (git-graph, serie) is Rust. |
| TUI | **[ratatui](https://ratatui.rs/)** + **crossterm** | De facto Rust TUI stack. Full mouse support (click, right-click, wheel), colors, cross-platform. |
| Git access | **The `git` CLI** (`std::process::Command`, `-z` / `%x1f` machine-readable formats) | Exactly the same behavior as the user's terminal: config, hooks, credential helpers, SSH, signing. No libgit2 differences. |
| Concurrency | Worker threads + channels (no async runtime) | Git calls run off the UI thread and can be cancelled. Simple and enough for a process-driven app. |
| CLI | `clap` | `tui-git-graph [PATH]`, flags for branch filter, all/remote branches. |
| Config | TOML at `~/.config/tui-git-graph/config.toml` (`serde`, `toml`) | Columns, date format, colors, keybindings, default filters. |
| Clipboard | OSC 52 (works over SSH, tmux, herdr), `arboard` fallback | Copy hash, subject, branch name. |
| Errors | `thiserror` in the core, `anyhow` in the binary | Typed errors where we match on them, context where we report them. |
| Testing | `cargo nextest`, `insta` snapshots, ratatui `TestBackend`, fixture repos built in temp dirs | Layout and rendering are snapshot-tested. Actions run against real throwaway repos. |
| Quality | `rustfmt`, `clippy -D warnings`, `cargo deny` | Enforced in CI. |
| Distribution | GitHub Releases (macOS, Linux, Windows) built by [cargo-dist](https://opensource.axo.dev/cargo-dist/), shell and PowerShell installers, Homebrew tap, `cargo install --path` | One binary, no runtime. |

## Architecture

A Cargo workspace (monorepo) of small crates with one-way dependencies. Pure crates never touch the terminal, and the UI never runs git directly.

| Crate | Path | Role | May depend on |
| --- | --- | --- | --- |
| `tgg-model` | `crates/model` | Domain types: commit ids, commits, refs, signatures. Pure data. | — |
| `tgg-git` | `crates/git` | Safe git argv building, process execution behind a trait, output parsers | model |
| `tgg-graph` | `crates/graph` | Lane layout: commits → rows of lanes/edges (incremental, stable across pages). Pure. | model |
| `tgg-actions` | `crates/actions` | Actions as data (argv, confirmation, refresh scope) and context menus per target | model, git |
| `tgg-ui` | `crates/ui` | ratatui widgets (graph, badges, details, menus, dialogs) and the per-frame hit-map | model, git (output types), graph, actions |
| `tui-git-graph` | `crates/app` | The binary: CLI, config, event loop, worker-thread runner, input → events | all of the above |
| `tgg-testkit` | `crates/testkit` | Throwaway fixture repos for tests (dev-dependency only) | — |

```
Cargo.toml              # workspace: members, shared package metadata, dependency catalog, lints, profiles
rust-toolchain.toml     # pinned toolchain (rustfmt, clippy)
crates/<crate>/         # one package per crate (see table)
integrations/herdr/     # herdr plugin: opens tui-git-graph in a herdr tab
integrations/ttt/       # tiny TTT plugin that runs tui-git-graph in a terminal tab (planned)
tests/fixtures/         # scripts/data for fixture repos
odd/tasks/              # ODD feature documents
```

**Data flow (unidirectional):**

```
input (key/mouse) ─▶ Event ─▶ update(state) ─▶ Command ─▶ runner (git, worker thread)
       ▲                                                          │
       └──────── render(state) ◀── update(state) ◀── Event::GitResult
```

- **Rendering is a pure function of state.** Each frame records a hit-map (which cell belongs to which row, badge or menu item), so mouse clicks and right-clicks resolve against exactly what was drawn.
- **Lane layout is incremental.** Loading more commits never changes rows already shown.
- **Actions are data.** Each action declares its git argv, whether it is destructive, its confirmation text and what to refresh. Menus are built from the selected target (commit, branch, tag, stash, uncommitted row), so the same action list drives mouse menus, keyboard menus and tests.

## Install

Prebuilt binaries for macOS (Apple Silicon and Intel), Linux (x86_64 and arm64) and Windows are attached to every [GitHub Release](https://github.com/allient/tui-git-graph/releases), starting with v0.1.0. Pick one:

```sh
# Homebrew (macOS, Linux)
brew install allient/tap/tui-git-graph

# macOS and Linux: the shell installer puts the binary in ~/.cargo/bin
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/allient/tui-git-graph/releases/latest/download/tui-git-graph-installer.sh | sh
```

```powershell
# Windows (PowerShell)
powershell -ExecutionPolicy Bypass -c "irm https://github.com/allient/tui-git-graph/releases/latest/download/tui-git-graph-installer.ps1 | iex"
```

Or download the archive for your platform from the release page, check it against its `.sha256` file, and put `tui-git-graph` on your `PATH`. Then run `tui-git-graph` inside a git repository, or `tui-git-graph ~/code/my-project`. It needs `git` on your `PATH`.

## Running it

To build it yourself, run it from a clone. Requires Rust (the toolchain is pinned in `rust-toolchain.toml`; `rustup` installs it automatically) and `git`.

```sh
git clone https://github.com/allient/tui-git-graph.git
cd tui-git-graph

cargo run                          # dev build, opens the repository you are in (this one)
cargo run -- ~/code/my-project     # dev build, opens another repository (any path inside it works)
cargo run --release -- ~/code/my-project   # optimized build: faster on large histories
```

Everything after `--` goes to the app. The first build takes a minute or two; later runs start right away. Outside a git repository it exits with an error.

To have the command on your `PATH` (installs to `~/.cargo/bin`):

```sh
cargo install --path crates/app    # then: tui-git-graph ~/code/my-project
```

### Keys and mouse

| Key / mouse | Does |
| --- | --- |
| `j` `k` / arrows, `PgUp` `PgDn`, `g` `G` | Move the selection (top, bottom) |
| `H` | Jump to the HEAD commit |
| `Enter` / left click | Open or close the commit's details; in the details, `j`/`k` pick a file, `Enter` opens its diff and `v` shows the whole file as it was in that commit |
| Click `#123` (in the details) | Issue references in the commit message are underlined; clicking one opens the issue in the browser: on origin's GitHub, GitLab or Bitbucket page, or the page set by `[issues] url` in the config file. `abc#12` is not a reference |
| `f` / find button (in the details) | Filter the changed files by name (ignoring case): type to narrow the list, `Backspace` deletes, `Enter` keeps the filter, `Esc` clears it. While the filter is edited every key types into it |
| `l` / tree and list buttons (in the details) | Show the changed files as a folder tree or as a flat list of full paths; the button of the layout shown is highlighted. The close button (or `Esc`) closes the details |
| `J` `K` / mouse wheel (in the details) | Scroll the commit's fields and message (`J` down, `K` up); the wheel scrolls whichever side it is over, message or files, and leaves the graph where it is |
| `t` (in the details) | Open the selected file in your configured `git difftool` (against the commit's first parent, or between the two sides of a comparison). The app steps aside until the tool exits |
| `s` (in a diff) | Show old and new side by side (old lines and numbers on the left, new on the right, deleted and added runs paired line by line), or back to one column. Needs a diff at least 100 columns wide; the choice holds for every diff until you press `s` again or quit |
| Diff pager | With a pager configured (`[diff] pager` in the config file, or `TUI_GIT_GRAPH_PAGER`), `Enter` on a file pipes its colored diff into it, for example `delta`, instead of the built-in diff view. The app steps aside until the pager exits; if it fails, the status bar says so |
| Double click a branch | Check the branch out (a remote branch asks for the local branch's name), like VS Code Git Graph |
| `m` / right click | Context menu for the commit, branch, tag, stash or uncommitted row |
| Remote branch menu | Also Delete Remote Branch... (asks first, `git push --delete`) and Pull into current branch... (`git pull <remote> <branch>`), as in VS Code |
| Fetch into local branch (remote branch menu) | Updates a local branch from the remote one without checking it out (`git fetch <remote> refs/heads/<branch>:refs/heads/<local>`). The dialog asks for the local branch's name (the remote branch's name by default; a new name creates the branch) and has a "Force fetch" toggle, which overwrites local commits after asking. Without force, only a fast-forward is allowed. Disabled for the current branch, which git can't fetch into: use Pull |
| Compare (commit menu) | "Select for Compare" marks a commit; on another commit, "Compare with Selected" lists the files changed from the marked one to it, and "Compare with Working Tree" lists what changed since the commit, uncommitted edits included. `Enter` opens a file's diff, `Esc` closes the comparison |
| Interactive Rebase from Here... (commit menu) | Edits the commits after the chosen one up to HEAD, like `git rebase -i <commit>`. Offered on an older commit of the current branch; disabled, with the reason, on HEAD itself, on a detached HEAD, when a merge commit lies between the commit and HEAD, or while an operation is in progress. A panel over the graph lists the commits oldest first (git's todo order) with their action: `p` pick, `s` squash (melds into the commit above, keeping both messages), `f` fixup (melds, keeping only the message above), `d` drop, `Space` cycles; `J`/`K` or `Alt+↓`/`Alt+↑` move the selected commit down or up; `j`/`k` or arrows select. The first kept commit can't be squashed or fixed up. The last line previews the result (`3 commits → 2`). `Enter` asks to confirm ("Rewrite N commits on '<branch>'? The old commits stay reachable in the reflog.") and runs the rebase without an editor; `Esc` cancels with nothing changed. Uncommitted changes to tracked files are refused ("Commit or stash your changes first.") instead of stashed. A conflict stops the rebase and the conflict banner takes over (Continue / Abort); a commit made since the panel opened stops it too, before anything is rewritten, so nothing is dropped by accident ("The branch changed after the rebase panel opened, so nothing was rewritten. Abort the rebase (A) and open the panel again.") |
| Show Reflog (repository menu) | Lists where HEAD has been, newest first (the latest 500 `git reflog` entries): short hash, `HEAD@{n}`, what moved HEAD (`commit: ...`, `checkout: moving from main to dev`, `reset: moving to HEAD~2`) and when. `j`/`k`, arrows, `PgUp`/`PgDn`, `Home`/`End` or a click select an entry; the wheel scrolls. `Enter` or right click opens its menu to recover the commit, even one a reset left on no branch: "Create Branch Here...", "Checkout (detached)...", "Reset current branch to here..." and "Copy Commit Hash to Clipboard". The view stays open and reloads after the action; `Esc` closes it |
| Worktrees... (repository menu) | Lists the repository's worktrees (`git worktree list`): a `*` on the one tui-git-graph runs in, short hash, branch (or `(detached)`, `(bare)`), folder, and `current`, `main`, `locked (reason)` or `prunable`. Selection, scrolling and clicks work as in the reflog view. `a` adds one: a branch, "Create a new branch" (on: `git worktree add -b <branch> -- <folder>` at HEAD; off: checks out an existing branch) and the folder (empty for `../<repository>-<branch>`). `Enter` or right click opens the worktree's menu: "Copy Path to Clipboard", "Lock"/"Unlock", "Remove Worktree..." (asks first, since it deletes the folder; "Force" also removes one with uncommitted changes or a lock; disabled for the main worktree and the current one), "Add Worktree..." and "Prune Worktrees" (forgets worktrees whose folder is gone). The view reloads after each action; `Esc` closes it |
| `Tab` | Move to the toolbar (branch filter, Show Remote Branches, `↻` Refresh, `⇣` Fetch; `...` repository menu when the Source Control pane is hidden); arrows move, `Enter` presses |
| `/` | Find: type to search the commits by message, author, date, hash, or branch or tag name; matches are highlighted. With no match among the loaded commits, older history is read page by page ("Searching older history…") until one turns up; "No matches" means none in the whole history. `Enter` or `↓` goes to the next match, reading older history before wrapping around past the last loaded one, `↑` to the previous one, `Esc` closes |
| `c` | Move to the Source Control pane on the left (shown from 90 columns). Type the commit message (`Alt+Enter`, `Shift+Enter` or `Ctrl+J` starts a new line) and press `Enter` to commit what is staged. `↓` moves to the Commit button and the files: `Enter` (or a click on the file's name) opens a file's diff, the working tree against the index under Changes and the index against HEAD under Staged Changes (in the pager, when one is configured); `Enter` on a section header, or `o`, opens "Open Changes": the diffs of every file of that section in one scrollable view, each under its own header with its status letter (always the built-in view); `e` opens the file in your editor (`$VISUAL`, else `$EDITOR`, else `vi`), which gets the terminal until it exits, then everything reloads (a deleted file can't be opened); `Space` stages or unstages a file or a whole section, as do `+` and `-`; a click on a section header, or `←` and `→` on it, collapses and expands the section (`▸` when collapsed), and its buttons still act on every file in it; `d` discards a file's changes (or the whole Changes section) after asking; `m` opens the repository menu; `Esc` goes back to the graph. The header's `✓` `↻` `…` commit, refresh and open the repository menu; each section header's `⧉` opens its changes; each row's `↗` `↶` `+` `−` open the file, discard, stage and unstage |
| `r` | Refresh |
| `C` / `A`, or the banner's `[Continue]` `[Abort]` | While a merge, rebase, cherry-pick or revert is stopped on a conflict, a banner under the toolbar says so and counts the conflicted files. `C` continues it (`git <operation> --continue`, keeping git's message) once every conflict is resolved and staged; `A` aborts it (`git <operation> --abort`) after asking. Until then, checkouts, merges, rebases, cherry-picks, reverts, resets, pulls and stash apply/pop are refused; staging, committing and discarding still work |
| Drag `│` | Drag the line between Source Control and the graph to resize the pane, or a divider in the graph's column header (Graph, Description, Date, Author, Commit) to resize the columns |
| Right click the column header | Show or hide the Date, Author and Commit columns (`✓` marks the shown ones), like VS Code Git Graph; the description takes the room. Narrow terminals still drop columns that don't fit (Commit under 60 columns, Date under 80, Author under 100) |
| `Esc` | Close the open menu, dialog, picker, diff or details; on the graph itself it quits |
| `q` | Quit (closes the diff first when one is open) |

Push, pull and fetch never prompt for a password: use SSH keys or a git credential helper. Without one, the command fails and the status bar shows git's error.

### Icons

The toolbar, the Source Control pane and the details panel's file tree show VS Code's own icons (Codicons, such as `cod-folder` and `cod-file` before folders and files, `cod-diff_multiple` for "Open Changes" and `cod-go_to_file` for "Open File", `cod-close`, `cod-search`, `cod-list_tree` and `cod-list_flat` for the details' buttons) when the terminal can draw them, and plain Unicode symbols otherwise (`[↻]` Refresh, `[⇣]` Fetch, `[...]` menu, `▾` folder, `·` file, `⧉` Open Changes, `↗` Open File, `✕` close, `⌕` find, `├` tree, `≡` list). Every icon button has a blank cell after it, so a Codicon drawn wider than its cell doesn't run into the next one. The default, `auto`, turns the icons on in Ghostty, WezTerm and Kitty, which ship the Nerd Font symbols, also inside tmux or herdr. Other terminals need a [Nerd Font](https://www.nerdfonts.com/) installed and selected, and the icons turned on by hand:

1. Install a Nerd Font, for example JetBrains Mono:

   ```sh
   brew install --cask font-jetbrains-mono-nerd-font   # macOS
   ```

   On Linux, download it from [nerdfonts.com/font-downloads](https://www.nerdfonts.com/font-downloads) and unzip it into `~/.local/share/fonts`, then run `fc-cache -f`.
2. Select the font in your terminal's settings (for example "JetBrainsMono Nerd Font"). WezTerm, Kitty and Ghostty already include the Nerd Font symbols, so you can skip steps 1 and 2 there.
3. Turn the icons on (not needed in Ghostty, WezTerm or Kitty), for one run or for every run:

   ```sh
   cargo run -- --icons nerd                       # one run
   export TUI_GIT_GRAPH_ICONS=nerd                 # every run (add it to ~/.zshrc or ~/.bashrc)
   ```

   `--icons unicode` always uses the plain symbols, `--icons auto` goes back to detecting the terminal, and the flag wins over the variable. If the icons show as empty boxes, the terminal isn't using a Nerd Font: use `--icons unicode`.

### Config file

Settings live in `~/.config/tui-git-graph/config.toml` (under `$XDG_CONFIG_HOME` when it is set, or wherever `TUI_GIT_GRAPH_CONFIG` points). The file is optional:

```toml
icons = "auto"            # "auto", "nerd" or "unicode"; --icons and TUI_GIT_GRAPH_ICONS win over it

[layout]                  # saved for you when you quit after resizing
source_control_width = 36
graph_column = 12
date_column = 16
author_column = 14
hash_column = 8
hidden_columns = ["author"] # hidden from the header's right-click menu: "date", "author", "commit"
changes_view = "tree"       # Source Control files: "list" (default) or "tree"
changes_sort = "name"       # "path" (default), "name" or "status"

[diff]
pager = "delta"           # pipe file diffs into this command; unset or "" uses the built-in view

[issues]                  # where issue references in commit messages link to
url = "https://jira.example.com/browse/PROJ-$1"   # $1 is replaced by the number
prefix = "PROJ-"          # what starts a reference; default "#"
```

Without `[issues] url`, `#123` opens issue 123 on origin's GitHub (`/issues/123`), GitLab (`/-/issues/123`) or Bitbucket (`/issues/123`) page; for any other origin, clicking a reference says how to set `url`. With `prefix = "PROJ-"`, `PROJ-123` is the reference and `#123` is plain text.

`pager` is a shell command (run with `sh -c`, `cmd /C` on Windows), so `"delta --side-by-side"` or `"less -R"` work too. `TUI_GIT_GRAPH_PAGER` wins over it, and setting it to an empty value turns a configured pager off for one run.

Drag the pane or column dividers, show and hide columns, or pick a View & Sort choice, and quit: the new layout is written to `[layout]` and used next time. Only the `[layout]` table is rewritten: other settings, tables, blank lines and comments elsewhere in the file are kept as they are, while comments inside `[layout]` are lost (a layout set with dotted keys such as `layout.date_column = 16` makes the whole file be rewritten, without its comments). A mistake in the file (an unknown setting or value) stops the app with a message naming the file and the line.

## Development

```sh
cargo run-app            # run the TUI (alias for `cargo run -p tui-git-graph --`)
cargo t                  # run all tests (cargo nextest)
cargo lint               # clippy on every crate and target, warnings are errors
cargo fmt --all          # format
cargo deny check         # licenses, advisories, sources
cargo machete            # unused dependencies
```

Tools: `cargo install cargo-nextest cargo-insta cargo-deny cargo-machete`.

Releasing: set the version in the root `Cargo.toml`, commit, then push a tag such as `git tag v0.1.0 && git push origin v0.1.0`. The release workflow (`.github/workflows/release.yml`, generated by `dist generate` from `dist-workspace.toml`) builds every platform, publishes the GitHub Release with archives, checksums and installers, and updates the formula in [allient/homebrew-tap](https://github.com/allient/homebrew-tap), which needs the `HOMEBREW_TAP_TOKEN` secret. `dist plan` shows what a release would contain; pull requests run only that plan.

**Dependencies.** Every version is declared once in `[workspace.dependencies]` in the root `Cargo.toml`. A crate opts in with `dep.workspace = true` (`cargo add -p <crate> <dep>` does this automatically) and declares only what it actually uses; `cargo machete` enforces that in CI.

## Roadmap

Steps 1 to 3 and step 5 are done; step 4 is in progress (the herdr plugin and the release pipeline are done).

1. **MVP:** colored graph with badges, uncommitted row, selection, scrolling, mouse + keyboard, branch and commit context menus with checkout / switch branch, create branch, tag, cherry-pick, revert, merge, reset, copy; commit details with file diffs.
2. **Daily driver:** branch filter, remote toggle, search, repository "..." menu, pull / push / fetch, tags and stashes, config file.
3. **Parity:** compare commits, archive, remotes management, issue linking, PR creation.
4. **Integrations and release:** herdr plugin (done), GitHub Releases and Homebrew tap (done, from v0.1.0), TTT plugin.
5. **Beyond:** interactive rebase, conflict handling, worktrees, reflog.

## Licensing

This project is MIT licensed. It is **inspired by** Git Graph for VS Code but contains **no code from it**; the Git Graph extension uses its own non-standard license. All implementation is original. MIT/Apache-2.0 projects listed below may be studied; anything substantial adapted from them is credited.

## Inspiration

| Project | Why it matters |
| --- | --- |
| [mhutchie/vscode-git-graph](https://github.com/mhutchie/vscode-git-graph) | The reference experience: graph plus actions from right-click menus. Our feature parity target. |
| [lusingander/serie](https://github.com/lusingander/serie) | Rust + ratatui git graph TUI (MIT). The closest existing tool: excellent rendering, read-only. Reference for drawing and terminal image/glyph handling. |
| [git-bahn/git-graph](https://github.com/git-bahn/git-graph) | Rust CLI (MIT) with a very clean lane layout per branching model. Reference for the graph layout algorithm. |
| [git-bahn/git-igitt](https://github.com/git-bahn/git-igitt) | Interactive TUI based on git-graph (MIT). Reference for browsing history and diffs in a terminal. |
| [the0807/git-graph-plus](https://github.com/the0807/git-graph-plus) | A modern, from-scratch Git Graph for VS Code (Apache-2.0). Ideas for stretch goals: interactive rebase, drag to merge, conflict prediction, reflog, worktrees. |
| [stablyai/orca#4521](https://github.com/stablyai/orca/issues/4521) | Feature request in the Orca agent IDE asking for exactly this. Shows real demand from users of terminal and agent-first tools. |
| [herdr](https://herdr.dev/) · [TTT editor](https://tttedit.dev/) | Host environments we integrate with through thin plugins. |
