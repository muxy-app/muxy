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

## `workspaceChanged`

Full workspace tree for one project's active worktree. See [Data Objects → Workspace](data-objects.md#workspace) for the recursive shape.
