# Events

Server-pushed events go to authenticated clients. Treat `workspaceChanged` as the main source of truth for layout updates.

| Event | Data type | Description |
| --- | --- | --- |
| `workspaceChanged` | `workspace` | Full workspace layout update |
| `tabChanged` | `tab` | Tab created, closed, selected, or retitled |
| `terminalOutput` | `terminalOutput` | Raw PTY bytes for a pane the client owns. Pushed as the shell/TUI writes. |
| `terminalSnapshot` | `terminalCells` | Full grid snapshot for a pane the client just took over. |
| `notificationReceived` | `notification` | New notification emitted by Muxy |
| `projectsChanged` | `projects` | Updated project list |
| `paneOwnershipChanged` | `paneOwnership` | Pane control changed between Mac and remote clients |
| `themeChanged` | `deviceTheme` | Updated terminal foreground/background colors |

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

The bytes are the exact sequence Ghostty read from the PTY on the Mac, before any terminal emulation. Feed them into your own VT emulator to render. There's no guarantee a chunk ends on a UTF-8 or escape-sequence boundary; the emulator must buffer partial sequences across chunks.

## `workspaceChanged`

Full workspace tree, keyed by `projectID + worktreeID`. See [Data Objects → Workspace](data-objects.md#workspace) for the recursive shape.
