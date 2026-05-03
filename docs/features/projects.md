# Projects

A project in Muxy is a directory on disk plus a bit of metadata (name, icon, color, last‑used IDE). Projects are how Muxy groups tabs, splits, and worktrees.

## Adding a project

- Click **+** at the bottom of the sidebar, or use **File → Open Project…** (`Cmd+O`).
- Drag a folder onto the Muxy dock icon.
- From a shell: `muxy /path/to/project` (after **Muxy → Install CLI**).
- Via URL scheme: `muxy://open?path=/path/to/project`.

All entry points dedupe — opening the same path twice just activates the existing project.

## Customising appearance

Right‑click a project in the sidebar to:

- **Rename** the project (display name only — does not move the folder).
- **Change icon**: emoji logo or letter badge.
- **Change color**: pick from the preset palette.
- **Remove** the project from Muxy (does not delete the folder).

## Switching projects

- **Next / Previous:** `Ctrl+]` / `Ctrl+[`.
- **Project 1–9:** `Ctrl+1…9`.
- Click any project in the sidebar.

Each project keeps its own tabs, splits, and active tab in memory while the app is running.

## Open in IDE

Muxy auto‑discovers IDE‑like apps installed on your Mac (VS Code, Zed, Sublime, JetBrains IDEs, Cursor, …). The **Open in IDE** topbar button and **File → Open in IDE** menu show what was found and remember your last choice. If an editor tab is active, the IDE is launched at that file's line and column when supported.

## CLI and URL scheme

The bundled `muxy-cli` binary is installed via **Muxy → Install CLI**:

```bash
muxy .
muxy /Users/me/projects/api
```

URL scheme handler:

```
muxy://open?path=/percent-encoded/path
```

Both routes call the same internal handler, so behaviour is identical.

## Persistence

Projects are stored as JSON at `~/Library/Application Support/Muxy/projects.json`. Tabs and splits are in‑memory only and lost on app close — use [Layouts](layouts.md) to define a reproducible workspace.

## Settings

- **General → Keep projects open after closing all tabs** keeps an empty project visible in the sidebar after its last tab is closed.
- **General → Auto‑expand worktrees on project switch** opens the worktree list when you switch to a project.
