# Usage reference

Every key, mouse action, icon setting and config option. The [documentation site](https://git-graph-tui.allient.io) has the same in shorter, illustrated pages.

## Keys and mouse

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
| Diff pager | With a pager configured (`[diff] pager` in the config file, or `GIT_GRAPH_TUI_PAGER`), `Enter` on a file pipes its colored diff into it, for example `delta`, instead of the built-in diff view. The app steps aside until the pager exits; if it fails, the status bar says so |
| Double click a branch | Check the branch out (a remote branch asks for the local branch's name), like VS Code Git Graph |
| `m` / right click | Context menu for the commit, branch, tag, stash or uncommitted row. Right-click a badge for its own menu; with `m` (or `Shift+F10`), each branch, remote branch and tag on the selected row is a submenu (`Branch main ▸`) above the commit's entries |
| Remote branch menu | Also Delete Remote Branch... (asks first, `git push --delete`) and Pull into current branch... (`git pull <remote> <branch>`), as in VS Code |
| Fetch into local branch (remote branch menu) | Updates a local branch from the remote one without checking it out (`git fetch <remote> refs/heads/<branch>:refs/heads/<local>`). The dialog asks for the local branch's name (the remote branch's name by default; a new name creates the branch) and has a "Force fetch" toggle, which overwrites local commits after asking. Without force, only a fast-forward is allowed. Disabled for the current branch, which git can't fetch into: use Pull |
| Compare (commit menu) | "Select for Compare" marks a commit; on another commit, "Compare with Selected" lists the files changed from the marked one to it, and "Compare with Working Tree" lists what changed since the commit, uncommitted edits included. `Enter` opens a file's diff, `Esc` closes the comparison |
| Interactive Rebase from Here... (commit menu) | Edits the commits after the chosen one up to HEAD, like `git rebase -i <commit>`. Offered on an older commit of the current branch; disabled, with the reason, on HEAD itself, on a detached HEAD, when a merge commit lies between the commit and HEAD, or while an operation is in progress. A panel over the graph lists the commits oldest first (git's todo order) with their action: `p` pick, `s` squash (melds into the commit above, keeping both messages), `f` fixup (melds, keeping only the message above), `d` drop, `Space` cycles; `J`/`K` or `Alt+↓`/`Alt+↑` move the selected commit down or up; `j`/`k` or arrows select. The first kept commit can't be squashed or fixed up. The last line previews the result (`3 commits → 2`). `Enter` asks to confirm ("Rewrite N commits on '<branch>'? The old commits stay reachable in the reflog.") and runs the rebase without an editor; `Esc` cancels with nothing changed. Uncommitted changes to tracked files are refused ("Commit or stash your changes first.") instead of stashed. A conflict stops the rebase and the conflict banner takes over (Continue / Abort); a commit made since the panel opened stops it too, before anything is rewritten, so nothing is dropped by accident ("The branch changed after the rebase panel opened, so nothing was rewritten. Abort the rebase (A) and open the panel again.") |
| Show Reflog (repository menu) | Lists where HEAD has been, newest first (the latest 500 `git reflog` entries): short hash, `HEAD@{n}`, what moved HEAD (`commit: ...`, `checkout: moving from main to dev`, `reset: moving to HEAD~2`) and when. `j`/`k`, arrows, `PgUp`/`PgDn`, `Home`/`End` or a click select an entry; the wheel scrolls. `Enter` or right click opens its menu to recover the commit, even one a reset left on no branch: "Create Branch Here...", "Checkout (detached)...", "Reset current branch to here..." and "Copy Commit Hash to Clipboard". The view stays open and reloads after the action; `Esc` closes it |
| Worktrees... (repository menu) | Lists the repository's worktrees (`git worktree list`): a `*` on the one git-graph-tui runs in, short hash, branch (or `(detached)`, `(bare)`), folder, and `current`, `main`, `locked (reason)` or `prunable`. Selection, scrolling and clicks work as in the reflog view. `a` adds one: a branch, "Create a new branch" (on: `git worktree add -b <branch> -- <folder>` at HEAD; off: checks out an existing branch) and the folder (empty for `../<repository>-<branch>`). `Enter` or right click opens the worktree's menu: "Copy Path to Clipboard", "Lock"/"Unlock", "Remove Worktree..." (asks first, since it deletes the folder; "Force" also removes one with uncommitted changes or a lock; disabled for the main worktree and the current one), "Add Worktree..." and "Prune Worktrees" (forgets worktrees whose folder is gone). The view reloads after each action; `Esc` closes it |
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

## Icons

The toolbar, the Source Control pane and the details panel's file tree show VS Code's own icons (Codicons, such as `cod-folder` and `cod-file` before folders and files, `cod-diff_multiple` for "Open Changes" and `cod-go_to_file` for "Open File", `cod-close`, `cod-search`, `cod-list_tree` and `cod-list_flat` for the details' buttons) when the terminal can draw them, and plain Unicode symbols otherwise (`[↻]` Refresh, `[⇣]` Fetch, `[...]` menu, `▾` folder, `·` file, `⧉` Open Changes, `↗` Open File, `✕` close, `⌕` find, `├` tree, `≡` list). Every icon button has a blank cell after it, so a Codicon drawn wider than its cell doesn't run into the next one. The default, `auto`, turns the icons on in Ghostty, WezTerm and Kitty, which ship the Nerd Font symbols, also inside tmux or herdr. Other terminals need a [Nerd Font](https://www.nerdfonts.com/) installed and selected, and the icons turned on by hand:

1. Install a Nerd Font, for example JetBrains Mono:

   ```sh
   brew install --cask font-jetbrains-mono-nerd-font   # macOS
   ```

   On Linux, download it from [nerdfonts.com/font-downloads](https://www.nerdfonts.com/font-downloads) and unzip it into `~/.local/share/fonts`, then run `fc-cache -f`.
2. Select the font in your terminal's settings (for example "JetBrainsMono Nerd Font"). WezTerm, Kitty and Ghostty already include the Nerd Font symbols, so you can skip steps 1 and 2 there.
3. Turn the icons on (not needed in Ghostty, WezTerm or Kitty), for one run or for every run:

   ```sh
   ggt --icons nerd                                # one run
   export GIT_GRAPH_TUI_ICONS=nerd                 # every run (add it to ~/.zshrc or ~/.bashrc)
   ```

   `--icons unicode` always uses the plain symbols, `--icons auto` goes back to detecting the terminal, and the flag wins over the variable. If the icons show as empty boxes, the terminal isn't using a Nerd Font: use `--icons unicode`.

## Config file

Settings live in `~/.config/git-graph-tui/config.toml` (under `$XDG_CONFIG_HOME` when it is set, or wherever `GIT_GRAPH_TUI_CONFIG` points). The file is optional:

```toml
icons = "auto"            # "auto", "nerd" or "unicode"; --icons and GIT_GRAPH_TUI_ICONS win over it

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
[refresh]                 # pick up changes made outside the app
watch = true              # reload when git's own files change (default true)
status_interval = 3       # seconds between working-tree checks while the terminal has focus; 0 turns them off
```

Without `[issues] url`, `#123` opens issue 123 on origin's GitHub (`/issues/123`), GitLab (`/-/issues/123`) or Bitbucket (`/issues/123`) page; for any other origin, clicking a reference says how to set `url`. With `prefix = "PROJ-"`, `PROJ-123` is the reference and `#123` is plain text.

`[refresh]` keeps the graph current without pressing `r`. A commit, checkout, branch, tag, stash, rebase or fetch made in another terminal or by your editor shows up within about a second: the app watches git's own files (`HEAD`, `index`, `refs/`), never your working tree, so it stays cheap even with `node_modules` or `target`. Where file events don't work (network drives, WSL `/mnt`, some containers), it compares those files' timestamps once a second instead. Edited files are found with `git status` every `status_interval` seconds while the terminal has focus, and right away when you switch back to it; on a repository where `git status` is slow, the checks are spaced out so they never take more than a tenth of the time. A reload keeps the selected commit and all the history already loaded.

`pager` is a shell command (run with `sh -c`, `cmd /C` on Windows), so `"delta --side-by-side"` or `"less -R"` work too. `GIT_GRAPH_TUI_PAGER` wins over it, and setting it to an empty value turns a configured pager off for one run.

Drag the pane or column dividers, show and hide columns, or pick a View & Sort choice, and quit: the new layout is written to `[layout]` and used next time. Only the `[layout]` table is rewritten: other settings, tables, blank lines and comments elsewhere in the file are kept as they are, while comments inside `[layout]` are lost (a layout set with dotted keys such as `layout.date_column = 16` makes the whole file be rewritten, without its comments). A mistake in the file (an unknown setting or value) stops the app with a message naming the file and the line.
