# API Methods

## Projects & Workspace

| Method | Parameters | Result |
| --- | --- | --- |
| `listProjects` | none | `projects` |
| `selectProject` | `projectID` | `ok` |
| `listWorktrees` | `projectID` | `worktrees` |
| `selectWorktree` | `projectID`, `worktreeID` | `ok` |
| `getWorkspace` | `projectID` | `workspace` |
| `createTab` | `projectID`, `areaID?`, `kind` | `tab` |
| `closeTab` | `projectID`, `areaID`, `tabID` | `ok` |
| `selectTab` | `projectID`, `areaID`, `tabID` | `ok` |
| `splitArea` | `projectID`, `areaID`, `direction`, `position` | `ok` |
| `closeArea` | `projectID`, `areaID` | `ok` |
| `focusArea` | `projectID`, `areaID` | `ok` |

Enums:

- `kind`: `terminal`, `vcs`, `editor`, `diffViewer`
- `direction`: `horizontal`, `vertical`
- `position`: `first`, `second`

## Terminal control

| Method | Parameters | Result |
| --- | --- | --- |
| `takeOverPane` | `paneID`, `cols`, `rows` | `ok` |
| `releasePane` | `paneID` | `ok` |
| `terminalInput` | `paneID`, `bytes` | `ok` |
| `terminalResize` | `paneID`, `cols`, `rows` | `ok` |
| `terminalScroll` | `paneID`, `deltaX`, `deltaY`, `precise` | `ok` |
| `getTerminalContent` | `paneID` | `terminalCells` |

Notes:

- Terminal control is **ownership-based**. Call `takeOverPane` before sending input or resize events; `releasePane` returns control to the Mac. If a pane is owned by another client, control requests may be ignored.
- `terminalInput.bytes` is base64-encoded raw bytes, delivered verbatim to the PTY. The client encodes escape sequences, control codes, and mouse reports directly.
- `getTerminalContent` is a **legacy pull API** that snapshots the rendered grid. New clients should render the pane with their own VT emulator and subscribe to the `terminalOutput` event stream instead.

## Notifications & visual data

| Method | Parameters | Result |
| --- | --- | --- |
| `getProjectLogo` | `projectID` | `projectLogo` |
| `listNotifications` | none | `notifications` |
| `markNotificationRead` | `notificationID` | `ok` |
| `subscribe` | `events` | `ok` |
| `unsubscribe` | `events` | `ok` |

`subscribe` / `unsubscribe` are accepted for compatibility, but clients should still be prepared to receive all broadcast event types.

## Git & worktrees

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

## Example: full authentication request

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
