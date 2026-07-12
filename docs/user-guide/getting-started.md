# Getting Started

A 2-minute tour from install to a working session.

```mermaid
flowchart TB
  Install[Install Muxy] --> CLI[Optional: Install CLI]
  CLI --> Add[Add a project]
  Install --> Add
  Add --> Tabs[Open tabs / splits]
  Tabs --> Worktree[Switch worktree]
  Tabs --> Rich[Rich Input<br/>⌘I]
```

## Requirements

- macOS 14 or newer (Apple Silicon or Intel)

## Install

1. Download the latest build from the releases page.
2. Drag `Muxy.app` to `/Applications` and launch it.
3. Optional: **Muxy -> Install CLI** writes a `muxy` wrapper to `/usr/local/bin/muxy`. If that location needs admin access and installation fails, Muxy falls back to `~/bin/muxy` or `~/.local/bin/muxy`.

## Add your first project

A project is just a directory you've added to Muxy.

1. Open the sidebar with `⌘B` (or **View → Toggle Sidebar**).
2. Click **+** at the bottom of the sidebar — or **File → Open Project…** (`⌘O`).
3. Type the folder name, choose the matching path, and press Return. Add parent names with a space or `/` to narrow results, such as `muxy Projects`. You can also type an explicit path such as `~/Projects/muxy`.
4. Right-click the project to rename, recolor, or change its icon.

Projects persist in `~/Library/Application Support/Muxy/projects.json`.

## Tabs & splits cheat sheet

| Action | Shortcut |
| --- | --- |
| New tab | `⌘T` |
| Rich input | `⌘I` |
| Split right / down | `⌘D` / `⌘⇧D` |
| Focus pane | `⌘⌥←/→/↑/↓` |
| Maximize pane | `⌘⌥↩` |
| Close pane / tab | `⌘⇧W` / `⌘W` |
| Switch tabs | `⌘1…9`, `⌘]` / `⌘[` |

Tabs can also hold a browser or an extension view. See [Tabs & Splits](../features/tabs-and-splits.md).

## Switching projects & worktrees

- **Project navigation**: `⌃]` / `⌃[`, or `⌃1…9`.
- **Switch worktree**: use the worktree picker on the project row (or the `switch-worktree` [CLI command](../features/muxy-cli.md)). Each worktree has its own tabs/splits.

## Configuring Ghostty

Muxy renders terminals through libghostty. Edit its Ghostty config from **Muxy -> Open Configuration...** and reload with `⌘⇧R`. See [Terminal](../features/terminal.md#configuration) for the config path and how Muxy seeds it on first launch.

## Next steps

- [Keyboard Shortcuts](keyboard-shortcuts.md)
- [Layouts](../layouts/overview.md) — reproducible per-project workspaces
- [Settings](settings.md) — every preference explained
