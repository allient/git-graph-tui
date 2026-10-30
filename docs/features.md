# Features and roadmap

The record of what git-graph-tui does, checked against the Git Graph extension for VS Code, and where it is going.

## Scope: feature parity checklist

Target feature set, based on Git Graph for VS Code. The detailed specs for the first release are in [`odd/tasks/git-graph-mvp.md`](../odd/tasks/git-graph-mvp.md).

### Graph view
- [x] Commit graph with color-coded lanes per branch and merge lines, drawn with thin lines (`│`) that turn once, on the commit's line, with rounded corners where a line changes lanes (`●─╮`, `●─╯`), in Git Graph's own palette (blue, magenta, green, orange, purple, red, cyan, ...)
- [x] Local and remote branches, tags, stashes and remote HEAD labels, as colored badges; each stash is its own row (`stash@{n}`) above the commit it was made on, with the stash menu on right-click
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
- [x] Clone: asks for a repository URL and a parent folder (the one holding this repository by default), clones into a folder named after the repository and says where it went; open that one with `git-graph-tui <folder>`
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

## Roadmap

Steps 1 to 3 and step 5 are done; step 4 is in progress (the herdr plugin and the release pipeline are done).

1. **MVP:** colored graph with badges, uncommitted row, selection, scrolling, mouse + keyboard, branch and commit context menus with checkout / switch branch, create branch, tag, cherry-pick, revert, merge, reset, copy; commit details with file diffs.
2. **Daily driver:** branch filter, remote toggle, search, repository "..." menu, pull / push / fetch, tags and stashes, config file.
3. **Parity:** compare commits, archive, remotes management, issue linking, PR creation.
4. **Integrations and release:** herdr plugin (done), GitHub Releases and Homebrew tap (done, from v0.1.0).
5. **Beyond:** interactive rebase, conflict handling, worktrees, reflog.
