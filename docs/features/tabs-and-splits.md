# Tabs & Splits

Every Muxy worktree owns a tree of split panes; each leaf pane holds a stack of tabs.

```mermaid
flowchart TB
  Worktree --> Root[SplitNode]
  Root --> Left[TabArea<br/>tabs: editor | shell]
  Root --> Right[Split: vertical]
  Right --> RT[TabArea<br/>tabs: logs]
  Right --> RB[TabArea<br/>tabs: btop | top]
```

Splits nest arbitrarily — the layout is a binary tree of horizontal and vertical splits.

## Tab kinds

| Kind | What it is |
| --- | --- |
| Terminal | A libghostty-powered terminal (the default) |
| Source Control | The git status / diff / branches / PRs view (`⌘K`) |
| Editor | Built-in syntax-highlighted file editor |
| Diff Viewer | Standalone single-file diff |

## Creating tabs

| How | Result |
| --- | --- |
| `⌘T` | New terminal tab |
| `⌘P` (Quick Open) | Editor tab for a file |
| File tree → right-click → Open | Editor tab |
| Source Control → click a changed file | Diff viewer tab |
| File menu → New Tab | New tab in the active pane |

## Renaming, pinning, coloring

- **Rename Tab** — `⌘⇧T`, or double-click the tab title.
- **Pin / Unpin** — `⌘⇧P`. Pinned tabs stay leftmost.
- Right-click → **Color** to apply an accent.
- Right-click → **Close Others / Close to the Left / Close to the Right**.

Custom titles and colors are saved in the workspace snapshot and survive worktree switches.

## Splits

| Action | Shortcut |
| --- | --- |
| Split Right | `⌘D` |
| Split Down | `⌘⇧D` |
| Close Pane | `⌘⇧W` |
| Focus Pane | `⌘⌥←/→/↑/↓` |
| Cycle Tab (All Panes) | `⌃Tab` / `⌃⇧Tab` |

## Drag and drop

Tabs can be dragged within a pane to reorder, between panes to move, or onto a pane edge to create a new split.

## Navigation history

Mouse side buttons (3 / 4) and three-finger horizontal trackpad swipes navigate Back / Forward through tab history. Keyboard equivalents: `⌘⌃←` / `⌘⌃→`.

## Persistence

The tab and split tree per worktree is in-memory only. To recreate a layout, use [Layouts](layouts/README.md).
