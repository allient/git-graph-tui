---
title: Keys and mouse
description: Every keyboard shortcut and mouse action.
---

## Graph

| Key / mouse | Does |
| --- | --- |
| `j` `k`, arrows, `PgUp` `PgDn`, `g` `G` | Move the selection (`g` top, `G` bottom) |
| `H` | Jump to the HEAD commit |
| `Enter` / left click | Open or close the commit's details |
| Double click a branch | Check the branch out |
| `m`, `Shift+F10` / right click | Context menu of the row, or of the badge you right-clicked |
| `/` | Find |
| `Tab` | Move to the toolbar (branch filter, remote branches, refresh, fetch) |
| `c` | Move to the Source Control pane |
| `r` | Refresh |
| `C` / `A` | Continue or abort a merge, rebase, cherry-pick or revert stopped on a conflict |
| Drag `│` | Resize the Source Control pane or a graph column |
| Right click the column header | Show or hide the Date, Author and Commit columns |
| `Esc` | Close the open menu, dialog, diff or details; on the graph, quit |
| `q` | Quit |

## Commit details

| Key | Does |
| --- | --- |
| `j` `k` | Pick a changed file |
| `Enter` | Open the file's diff (or pipe it into your pager) |
| `v` | Show the whole file as it was in that commit |
| `t` | Open the file in `git difftool` |
| `f` | Filter the files by name |
| `l` | Switch between a folder tree and a flat list |
| `J` `K` / wheel | Scroll the message and fields |
| `s` (in a diff) | One column or side by side |

## Source Control

| Key | Does |
| --- | --- |
| `Enter` (message box) | Commit what is staged |
| `Alt+Enter`, `Shift+Enter`, `Ctrl+J` | New line in the message |
| `↓` | Move to the Commit button and the files |
| `Space`, `+`, `-` | Stage or unstage a file or a section |
| `Enter` / `o` | Open a file's diff / Open Changes for a section |
| `e` | Open the file in your editor |
| `d` | Discard changes, after asking |
| `←` `→` (section header) | Collapse or expand the section |
| `m` | Repository menu |
| `Esc` | Back to the graph |

## Find

| Key | Does |
| --- | --- |
| `Enter`, `↓` | Next match |
| `↑` | Previous match |
| `Esc` | Close |

## Interactive rebase panel

| Key | Does |
| --- | --- |
| `p` `s` `f` `d` | Pick, squash, fixup, drop |
| `Space` | Cycle the action |
| `J` `K`, `Alt+↓` `Alt+↑` | Move the selected commit down or up |
| `Enter` | Confirm and run the rebase |
| `Esc` | Cancel, nothing changed |
