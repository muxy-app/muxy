# Remote Server API

Muxy exposes a WebSocket API that lets external clients connect to the desktop app over the local network.

This API is intended for mobile apps, dashboards, companion tools, and custom integrations.

## Overview

- Protocol: WebSocket
- Endpoint: `ws://<host>:<port>`
- Default port: `4865`
- Message format: JSON
- Character encoding: UTF-8
- Date format: ISO 8601
- Identifier format: UUID strings

The server is disabled by default and must be enabled in Muxy's Mobile settings on macOS.

## Security Model

The current API is designed for trusted local networks.

- Transport is `ws://`, not TLS
- Clients must authenticate before using the API
- New devices must be approved from the Mac before they become trusted

For production integrations, treat the connection as local-network only unless you provide your own secure tunnel such as Tailscale or a VPN.

## Authentication and Pairing

Each client should generate and persist these values:

- `deviceID`: a stable UUID for that client install
- `deviceName`: a user-friendly device label
- `token`: a random secret persisted securely on the client

Connection flow:

1. Connect to the WebSocket endpoint.
2. Send `authenticateDevice`.
3. If the server returns `401`, send `pairDevice`.
4. The user approves the device in Muxy on macOS.
5. On success, the server returns a `clientID` for the active session.

Until authentication succeeds, all other API methods return `401 Authentication required`.

## Message Model

Every WebSocket frame is a JSON object with a top-level `type` field.

Supported message types:

- `request`
- `response`
- `event`

### Request Envelope

```json
{
  "type": "request",
  "payload": {
    "id": "request-id",
    "method": "listProjects",
    "params": null
  }
}
```

### Response Envelope

Success:

```json
{
  "type": "response",
  "payload": {
    "id": "request-id",
    "result": {
      "type": "ok"
    }
  }
}
```

Failure:

```json
{
  "type": "response",
  "payload": {
    "id": "request-id",
    "error": {
      "code": 401,
      "message": "Authentication required"
    }
  }
}
```

Only one of `result` or `error` is present on a given response; the unused field is omitted from the JSON.

### Event Envelope

```json
{
  "type": "event",
  "payload": {
    "event": "workspaceChanged",
    "data": {
      "type": "workspace",
      "value": {
        "projectID": "9b84c9a0-1d55-4c64-bbf6-ef59ee02fa09",
        "worktreeID": "ef8d7324-5b0d-4fe7-8d87-4f9d6f8106e2",
        "focusedAreaID": "d62a57b7-eb66-42d8-9d18-54d8c603ca7d",
        "root": {
          "type": "tabArea",
          "tabArea": {
            "id": "d62a57b7-eb66-42d8-9d18-54d8c603ca7d",
            "projectPath": "/Users/example/project",
            "tabs": [],
            "activeTabID": null
          }
        }
      }
    }
  }
}
```

## Request Format

Requests use this shape:

```json
{
  "id": "request-id",
  "method": "getWorkspace",
  "params": {
    "type": "getWorkspace",
    "value": {
      "projectID": "9b84c9a0-1d55-4c64-bbf6-ef59ee02fa09"
    }
  }
}
```

Rules:

- `id` must be unique per in-flight request
- `method` identifies the API operation
- `params.type` must match `method` when params are present
- methods without parameters may send `params: null`

## Error Codes

| Code | Meaning |
| --- | --- |
| `400` | Invalid parameters |
| `401` | Authentication required |
| `403` | Pairing denied |
| `404` | Resource not found |
| `408` | Pairing request timed out |
| `500` | Internal error or operation failure |

## Authentication Methods

### `authenticateDevice`

Authenticates a previously approved device.

Request:

```json
{
  "type": "authenticateDevice",
  "value": {
    "deviceID": "2f8d1f9f-e065-4f62-af30-8c4b3d0bfc53",
    "deviceName": "Pixel 9",
    "token": "random-secret-token"
  }
}
```

Success result:

```json
{
  "type": "pairing",
  "value": {
    "clientID": "62ea9d06-a1f4-4a11-9f39-33ee322f6573",
    "deviceName": "Pixel 9",
    "themeFg": 16777215,
    "themeBg": 197379,
    "themePalette": [0, 16711680, 65280]
  }
}
```

`themeFg`, `themeBg`, and `themePalette` are optional and may be omitted.

### `pairDevice`

Requests approval for a new device.

Request shape is the same as `authenticateDevice`.

Success returns the same `pairing` result.

### `registerDevice`

Registers a transient session for a device that has not persisted credentials. The server returns a `deviceInfo` result with the same fields as `pairing` (`clientID`, `deviceName`, optional `themeFg`, `themeBg`, `themePalette`).

Request:

```json
{
  "type": "registerDevice",
  "value": {
    "deviceName": "Pixel 9"
  }
}
```

## Recommended Client Startup Flow

1. Connect.
2. Authenticate or pair.
3. Call `listProjects`.
4. Choose a project.
5. Call `listWorktrees`.
6. Call `selectProject` and optionally `selectWorktree`.
7. Call `getWorkspace`.
8. Optionally load notifications, logos, and VCS state.

## API Methods

### Projects and Workspace

| Method | Parameters | Result |
| --- | --- | --- |
| `listProjects` | none | `projects` |
| `selectProject` | `projectID` | `ok` |
| `listWorktrees` | `projectID` | `worktrees` |
| `selectWorktree` | `projectID`, `worktreeID` | `ok` |
| `getWorkspace` | `projectID` | `workspace` |
| `createTab` | `projectID`, `areaID` optional, `kind` | `tab` |
| `closeTab` | `projectID`, `areaID`, `tabID` | `ok` |
| `selectTab` | `projectID`, `areaID`, `tabID` | `ok` |
| `splitArea` | `projectID`, `areaID`, `direction`, `position` | `ok` |
| `closeArea` | `projectID`, `areaID` | `ok` |
| `focusArea` | `projectID`, `areaID` | `ok` |

Valid enum values:

- `kind`: `terminal`, `vcs`, `editor`, `diffViewer`
- `direction`: `horizontal`, `vertical`
- `position`: `first`, `second`

### Terminal Control

| Method | Parameters | Result |
| --- | --- | --- |
| `takeOverPane` | `paneID`, `cols`, `rows` | `ok` |
| `releasePane` | `paneID` | `ok` |
| `terminalInput` | `paneID`, `bytes` | `ok` |
| `terminalResize` | `paneID`, `cols`, `rows` | `ok` |
| `terminalScroll` | `paneID`, `deltaX`, `deltaY`, `precise` | `ok` |
| `getTerminalContent` | `paneID` | `terminalCells` |

Notes:

- Terminal control is ownership-based.
- A client should call `takeOverPane` before sending input or resize events.
- `releasePane` returns control to the Mac.
- If the pane is owned by another client, control requests may be ignored.
- `terminalInput` carries raw bytes (base64-encoded on the JSON wire) that are
  delivered verbatim to the PTY, so the client is responsible for encoding
  escape sequences, control codes, and mouse reports directly.
- `getTerminalContent` is a legacy pull API that snapshots the rendered grid.
  New clients should render the pane with their own VT emulator and subscribe
  to the `terminalOutput` event stream instead.

### Notifications and Visual Data

| Method | Parameters | Result |
| --- | --- | --- |
| `getProjectLogo` | `projectID` | `projectLogo` |
| `listNotifications` | none | `notifications` |
| `markNotificationRead` | `notificationID` | `ok` |
| `subscribe` | `events` | `ok` |
| `unsubscribe` | `events` | `ok` |

`subscribe` and `unsubscribe` are accepted for compatibility, but clients should still be prepared to receive all broadcast event types.

### Git and Worktrees

| Method | Parameters | Result |
| --- | --- | --- |
| `getVCSStatus` | `projectID` | `vcsStatus` |
| `vcsCommit` | `projectID`, `message`, `stageAll` | `ok` |
| `vcsPush` | `projectID` | `ok` |
| `vcsPull` | `projectID` | `ok` |
| `vcsStageFiles` | `projectID`, `paths` | `ok` |
| `vcsUnstageFiles` | `projectID`, `paths` | `ok` |
| `vcsDiscardFiles` | `projectID`, `paths`, `untrackedPaths` | `ok` |
| `vcsListBranches` | `projectID` | `vcsBranches` |
| `vcsSwitchBranch` | `projectID`, `branch` | `ok` |
| `vcsCreateBranch` | `projectID`, `name` | `ok` |
| `vcsCreatePR` | `projectID`, `title`, `body`, `baseBranch`, `draft` | `vcsPRCreated` |
| `vcsAddWorktree` | `projectID`, `name`, `branch`, `createBranch` | `worktrees` |
| `vcsRemoveWorktree` | `projectID`, `worktreeID` | `ok` |

## Events

The server can push these event names:

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

For most clients, `workspaceChanged` should be treated as the main source of truth for layout updates.

### `terminalOutput` Event

Pushed only to the client that currently owns the pane. Payload:

```json
{
  "type": "terminalOutput",
  "value": {
    "paneID": "uuid",
    "bytes": "<base64-encoded raw PTY bytes>"
  }
}
```

The bytes are the exact sequence Ghostty read from the PTY on the Mac, before
any terminal emulation. A client should feed them into its own VT emulator
(e.g. SwiftTerm's `feed(byteArray:)`) to render the pane. There is no guarantee
that a chunk ends on a UTF-8 boundary or an escape-sequence boundary; the
emulator is expected to buffer partial sequences across chunks.

## Data Objects

### Project

```json
{
  "id": "uuid",
  "name": "muxy",
  "path": "/Users/example/project",
  "sortOrder": 0,
  "createdAt": "2026-04-19T10:00:00Z",
  "icon": "hammer",
  "logo": "custom",
  "iconColor": "#7C3AED"
}
```

### Worktree

```json
{
  "id": "uuid",
  "name": "main",
  "path": "/Users/example/project",
  "branch": "main",
  "isPrimary": true,
  "canBeRemoved": false,
  "createdAt": "2026-04-19T10:00:00Z"
}
```

### Workspace

A workspace contains:

- `projectID`
- `worktreeID`
- `focusedAreaID`
- `root`

`root` is a recursive tree with two node types:

- `tabArea`
- `split`

A `tabArea` contains:

- `id`
- `projectPath`
- `tabs`
- `activeTabID`

A tab contains:

- `id`
- `kind`
- `title`
- `isPinned`
- `paneID`

`paneID` is required for terminal-related methods.

### Terminal Snapshot

`getTerminalContent` returns a full terminal grid:

```json
{
  "paneID": "uuid",
  "cols": 120,
  "rows": 40,
  "cursorX": 10,
  "cursorY": 5,
  "cursorVisible": true,
  "defaultFg": 16777215,
  "defaultBg": 0,
  "cells": [
    {
      "codepoint": 65,
      "fg": 16777215,
      "bg": 0,
      "flags": 0
    }
  ]
}
```

Notes:

- colors are integer RGB values in `0xRRGGBB` form
- `cells` is a flat array representing the full terminal grid
- `flags` is a bitmask for text styling and wide-character metadata

### Notification

A notification includes:

- `id`
- `paneID`
- `projectID`
- `worktreeID`
- `areaID`
- `tabID`
- `source`
- `title`
- `body`
- `timestamp`
- `isRead`

This allows clients to link a notification back to the exact pane and tab that produced it.

### Project Logo

Project logos are returned as Base64-encoded PNG data.

```json
{
  "projectID": "uuid",
  "pngData": "iVBORw0KGgoAAAANS..."
}
```

## Example Authentication Request

```json
{
  "type": "request",
  "payload": {
    "id": "1",
    "method": "authenticateDevice",
    "params": {
      "type": "authenticateDevice",
      "value": {
        "deviceID": "2f8d1f9f-e065-4f62-af30-8c4b3d0bfc53",
        "deviceName": "Android Client",
        "token": "random-secret-token"
      }
    }
  }
}
```

## Integration Recommendations

- Persist `deviceID` and `token` securely
- Re-authenticate after reconnecting
- Treat `workspaceChanged` as authoritative
- Cache project logos after decoding the Base64 payload
- Call `takeOverPane` before interactive terminal control
- Handle `401` by retrying with pairing only when appropriate
- Do not assume event filtering is enforced server-side
