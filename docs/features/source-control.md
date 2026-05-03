# Source Control

Muxy ships a full git UI for the active worktree. Open it with `Cmd+K`, or from **File → Source Control**.

## Display modes

Source Control can render in three places (configurable in **Settings → General**):

- **Tab** — opens as a regular workspace tab.
- **Attached** — slides in as a side panel on the main window.
- **Window** — opens in a separate "Source Control" window (id `vcs`).

## Status

Files are grouped into:

- **Staged** — what `git commit` will record.
- **Changes** — modified, tracked files.
- **Untracked** — new, unignored files.

Toggle between flat list and folder tree. Stage / unstage individual files or whole directories. Discard changes is available from the right‑click menu.

## Diffs

Click a file to see its diff inline. The diff supports:

- **Unified** and **Split** views (toggle in the toolbar).
- Syntax highlighting.
- Collapsible context lines.
- Hover blame (toggle on) showing author and date for each line.

For deeper inspection, **Open in Diff Viewer** opens the file as a standalone diff tab.

## Commit, push, pull

- Type a message in the commit box; **Commit** with `Cmd+Return` (auto‑stage toggle picks up unstaged changes if enabled).
- **Push** uploads to the upstream branch and shows ↑N when ahead. Pushing a branch with no upstream prompts to set one.
- **Pull** fetches and merges; shows ↓N when behind.

## Branches and worktrees

The branch dropdown lets you switch branches (Muxy refuses if there are uncommitted changes). The **Create Branch…** sheet creates and checks out a new branch. The worktree picker is shared with the topbar control — see [Worktrees](worktrees.md).

## Pull requests

If the project's `origin` is on GitHub and `gh` is authenticated, Muxy shows:

- **PR pill** in the header (state, base, mergeability).
- **Pull Requests** section with search, state filter (Open/Closed/Merged/All), and manual or interval‑based auto‑sync (Off / 5m / 15m / 30m / 1h).
- **Create PR…** sheet with branch strategy, draft toggle, and "Open in browser after creation".
- Per‑PR actions: open on GitHub, merge, close, refresh.

## History

The Commit History section lists recent commits chronologically. Right‑click a commit for **Show Diff**, **Copy Hash**, and other actions.

## Layout

The Staged / Changes / History / Pull Requests sections are vertically resizable; their split ratios persist per project.
