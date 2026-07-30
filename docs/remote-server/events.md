# Events

The server pushes events to every authenticated client. `subscribe` / `unsubscribe` are accepted but do nothing — there is no server-side filtering, so a client must be ready to receive every event type below. Treat `workspaceChanged` as the source of truth for tab and layout state.

| Event | `data.type` | Description |
| --- | --- | --- |
| `workspaceChanged` | `workspace` | Full workspace tree for one project's active worktree. Pushed when tabs, splits, focus, titles, or pin/color state change. One event per active project per change burst (debounced ~80 ms). |
| `terminalOutput` | `terminalOutput` | Raw PTY bytes for a pane the client owns. Pushed as the shell/TUI writes. |
| `terminalSnapshot` | `terminalSnapshot` | A synthesized repaint of a pane the client just took over (see below). |
| `notificationReceived` | `notification` | A new notification emitted by Muxy. |
| `projectsChanged` | `projects` | Updated project list. Pushed when projects are added, removed, renamed, reordered, or have their icon/logo/color changed (debounced ~80 ms). |
| `paneOwnershipChanged` | `paneOwnership` | Pane control moved between the Mac and a remote client. |
| `themeChanged` | `deviceTheme` | Updated terminal foreground/background/palette colors. |
| `fileChanged` | `fileChanged` | Files changed by a file RPC, or on disk in the active local worktree. |

> `terminalOutput` and `terminalSnapshot` carry the same data shape — `{ paneID, bytes }` — but each uses its own `data.type` (matching its event name). They differ in what the bytes contain: raw PTY bytes vs. a synthesized repaint.

## `terminalOutput`

Pushed only to the client that currently owns the pane.

```json
{
  "type": "terminalOutput",
  "value": {
    "paneID": "uuid",
    "bytes": "<base64-encoded raw PTY bytes>"
  }
}
```

The bytes are the exact sequence Ghostty read from the PTY on the Mac, before any terminal emulation. Feed them into your own VT emulator to render. A chunk is not guaranteed to end on a UTF-8 or escape-sequence boundary; the emulator must buffer partial sequences across chunks.

## `terminalSnapshot`

Pushed once to the client immediately after a successful `takeOverPane`, so it can paint the current screen before live output arrives.

```json
{
  "type": "terminalSnapshot",
  "value": {
    "paneID": "uuid",
    "bytes": "<base64-encoded ANSI repaint>"
  }
}
```

Unlike `terminalOutput`, these bytes are **synthesized** by the desktop from the current grid: a clear-screen, cursor-home, and per-cell SGR sequences that reproduce the visible screen (it switches the alt screen on first when the pane is in alt mode). Feed them into the same VT emulator as `terminalOutput`. If the pane has no renderable content the event is skipped.

## `paneOwnershipChanged`

```json
{
  "type": "paneOwnership",
  "value": {
    "paneID": "uuid",
    "owner": { "remote": { "deviceID": "uuid", "deviceName": "Pixel 9" } }
  }
}
```

`owner` is a tagged object: `{ "mac": { "deviceName": "…" } }` when the Mac holds the pane, or `{ "remote": { "deviceID": "…", "deviceName": "…" } }` when a client does.

## `themeChanged`

```json
{
  "type": "deviceTheme",
  "value": { "fg": 16777215, "bg": 197379, "palette": [0, 16711680, 65280] }
}
```

`palette` is optional. Colors are integer RGB in `0xRRGGBB` form.

## `fileChanged`

```json
{
  "type": "fileChanged",
  "value": {
    "projectID": "uuid",
    "worktreeID": "uuid",
    "paths": ["src/main.swift", "README.md"],
    "truncated": false
  }
}
```

Pushed after a successful file mutation RPC or when the filesystem watcher sees changes under the active local worktree root, so a client rendering a file tree can refresh instead of polling [`filesList`](methods.md).

- `paths` are **relative to the worktree root**, matching the file methods, and sorted. Each mutation RPC emits one immediate batch containing all affected paths. Watcher changes are debounced and emitted in batches rather than one event per file.
- `worktreeID` names the worktree the paths belong to; it is optional and omitted when the project has no active worktree. Discard the event if it does not match the worktree you are displaying — the Mac may have switched worktrees under you, and the file methods always target the active one.
- The batch is capped at 200 paths. When more change at once — a branch switch, a large checkout — `truncated` is `true` and the listed paths are only part of the change: refresh the tree wholesale rather than patching those entries.
- Paths under `.git/` are filtered out; they are not listable through the file API anyway.
- The event reports *that* paths changed, not how. A path may have been created, modified, or deleted — call `filesStat` or `filesList` if you need to know which.

Every successful `filesWrite`, `filesMkdir`, `filesRename`, `filesMove`, or `filesDelete` emits paths for the requested project and worktree, including background local projects and SSH workspaces. Rename and move events include both the old and new paths.

**Watcher coverage limit.** Changes made outside the file RPCs are watched only in the **active local** project's worktree. External changes in background projects or SSH workspaces do not emit `fileChanged`; do not treat silence as "nothing changed" there.

## `workspaceChanged`

Full workspace tree for one project's active worktree. See [Data Objects → Workspace](data-objects.md#workspace) for the recursive shape.
