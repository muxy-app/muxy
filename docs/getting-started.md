# Getting Started

Muxy is a macOS terminal multiplexer focused on per‑project workspaces. This guide gets you from install to a working layout.

## Requirements

- macOS 14 or newer
- Apple Silicon or Intel

## Install

Download the latest build from the project's releases page, drag `Muxy.app` to `/Applications`, and launch it. On first launch macOS may prompt to confirm the developer.

Optional CLI install (from the menu bar): **Muxy → Install CLI**. This installs a `muxy` command at `/usr/local/bin/muxy` so you can do `muxy /path/to/project` from any shell.

## Add your first project

A project is a directory you've added to Muxy.

1. Open the sidebar (**View → Toggle Sidebar**, or `Cmd+B`).
2. Click the **+** button at the bottom of the sidebar — or use **File → Open Project…** (`Cmd+O`) — and pick a directory.
3. The project appears in the sidebar with a letter badge. Right‑click the project to rename, recolor, or change its icon.

Projects persist between launches in `~/Library/Application Support/Muxy/projects.json`.

## Working with tabs and splits

- **New tab:** `Cmd+T`. Each tab is a terminal by default.
- **Split right:** `Cmd+D`. **Split down:** `Cmd+Shift+D`.
- **Focus another pane:** `Cmd+Opt+←/→/↑/↓`.
- **Close pane / tab:** `Cmd+Shift+W` / `Cmd+W`.
- **Switch tabs:** `Cmd+1…9`, or `Cmd+]` / `Cmd+[`.

Tabs can also be source‑control views, file editors, or diff viewers — see [Tabs & Splits](features/tabs-and-splits.md).

## Switching projects

- **Next/previous project:** `Ctrl+]` / `Ctrl+[`.
- **Project 1–9:** `Ctrl+1…9`.
- Each project keeps its own tabs and splits. State is in‑memory per session.

## Worktrees

If a project has git worktrees, Muxy shows a worktree picker per project. **Switch Worktree:** `Cmd+Shift+O`. Each worktree has its own tabs and splits. See [Worktrees](features/worktrees.md).

## Source Control

Open a Source Control view with `Cmd+K`. You get staged/unstaged/untracked file lists, a commit box, branch and worktree controls, and inline diffs. See [Source Control](features/source-control.md).

## Configuring Ghostty

The terminal is rendered by libghostty. Its configuration lives at `~/.config/ghostty/config` and you can edit it with **Muxy → Open Configuration…** Reload it with `Cmd+Shift+R`.

## Next steps

- [Keyboard Shortcuts](keyboard-shortcuts.md)
- [Layouts](features/layouts.md) — declare reusable per‑project workspaces
- [Settings](settings.md) — every preference explained
