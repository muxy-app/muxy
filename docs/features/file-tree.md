# File Tree

The file tree is a side panel showing the active worktree's directory structure. Toggle it with `Cmd+E`.

## What it shows

- Lazy‑loaded directories — children load when you expand a folder.
- `.gitignore` is respected via `git check-ignore`. Toggle ignored files visibility from the panel header.
- **Show only changes** filters the tree to files with git changes.
- The currently active editor file is highlighted; its parent folders are auto‑expanded.

## Git status colors

Files are colored by git status: modified, added, untracked, deleted. Folders inherit a status hint from their descendants.

## File operations

Right‑click any item, or use the keyboard:

- **New File / New Folder** — inline text field on the parent folder.
- **Rename** — double‑click or right‑click → Rename.
- **Delete** — moves to Trash via `NSWorkspace.recycle`.
- **Cut / Copy / Paste** — uses the system pasteboard with a Muxy cut marker.
- **Reveal in Finder**.
- **Open in Terminal** — creates a new terminal tab at that directory.

Multi‑select with `Cmd+Click` and `Shift+Click`. Drag and drop moves files; hold `Option` while dragging to copy.

## External changes

A FSEvents watcher picks up changes made outside Muxy — no manual refresh needed.

## Resizing

The panel width is draggable and persists in user defaults across launches.
