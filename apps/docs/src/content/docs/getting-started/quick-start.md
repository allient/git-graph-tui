---
title: Quick start
description: Open a repository and learn the basics in a minute.
---

Run it inside a git repository, or pass the path to one (any folder inside it works):

```sh
git-graph-tui
git-graph-tui ~/code/my-project
```

Outside a git repository it exits with an error.

## The basics

| Do this | With |
| --- | --- |
| Move the selection | `j` `k` or the arrows, or click a row |
| Open a commit's details | `Enter` or a click |
| Open a context menu | Right-click, or `m` |
| Find a commit | `/` |
| Write a commit | `c` |
| Refresh | `r` |
| Quit | `q` (or `Esc` on the graph) |

See [Keys and mouse](/reference/keys/) for every shortcut.

## Network operations

Push, pull and fetch never prompt for a password. Use SSH keys or a [git credential helper](https://git-scm.com/docs/gitcredentials). Without one, the command fails and the status bar shows git's error.
