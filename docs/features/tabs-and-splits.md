# Tabs & Splits

Every Muxy worktree owns a tree of split panes; each leaf pane holds a stack of tabs.

## Tab kinds

A tab can be one of:

- **Terminal** — a libghostty‑powered terminal (the default).
- **Source Control** — the git status / diff / branches / PRs view (`Cmd+K`).
- **Editor** — a built‑in syntax‑highlighted file editor.
- **Diff Viewer** — a standalone single‑file diff.

## Creating tabs

- **New Tab:** `Cmd+T` — opens a terminal in a new tab.
- **Quick Open** (`Cmd+P`) — opens a file in a new editor tab.
- Right‑click a file in the file tree → **Open** to open as an editor tab.
- Click a changed file in the Source Control view → opens a diff viewer tab.
- File menu → **New Tab** in active pane.

## Renaming, pinning, coloring

- **Rename Tab:** `Cmd+Shift+T`, or double‑click the tab title.
- **Pin / Unpin Tab:** `Cmd+Shift+P`. Pinned tabs stay leftmost.
- Right‑click a tab → **Color** to apply an accent.
- Right‑click a tab for **Close Others / Close to the Left / Close to the Right**.

Custom titles and colors are saved in the workspace snapshot and survive worktree switches.

## Splits

- **Split Right:** `Cmd+D` (vertical divider, new pane to the right).
- **Split Down:** `Cmd+Shift+D` (horizontal divider, new pane below).
- **Close Pane:** `Cmd+Shift+W`.
- **Focus Pane:** `Cmd+Opt+←/→/↑/↓`.

Splits nest arbitrarily — the layout is a binary tree of horizontal and vertical splits.

## Drag and drop

Tabs can be dragged:

- Within a pane to reorder.
- Between panes to move.
- Onto a pane edge to create a new split.

## Navigation history

The mouse side buttons (3 / 4) and three‑finger horizontal trackpad swipes navigate Back / Forward through your tab history. Keyboard equivalents: `Cmd+Ctrl+←` / `Cmd+Ctrl+→`.

## Persistence

The tab and split tree per worktree is in‑memory only. To recreate a layout, use [Layouts](layouts.md) — a declarative `.muxy/layouts/<name>.yaml` file you can apply on demand or as `.muxy/startup.yaml` on first project open.
