# API Methods

Each method name doubles as the `params.type` discriminator. The **Result** column is the `result.type` returned on success (see [Protocol](protocol.md)). Only `authenticateDevice` and `pairDevice` are reachable before authentication; every other method (including `registerDevice`) requires an authenticated client and otherwise returns `401`.

## Projects & Workspace

| Method | Parameters | Result |
| --- | --- | --- |
| `listProjects` | none | `projects` |
| `listWorkspaces` | none | `workspaces` |
| `listProjectsByWorkspace` | `workspaceID` | `projects` |
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

- `kind`: `terminal` or `vcs`. `extensionWebView` is rejected — only terminal and VCS tabs can be created remotely.
- `direction`: `horizontal`, `vertical`
- `position`: `first`, `second`

`listWorkspaces` returns every workspace as a [workspace info](data-objects.md) object — the implicit **Local** workspace (`isDefault: true`) that owns all ungrouped local projects plus Home, each user-defined local group, and each remote (SSH) workspace. `listProjectsByWorkspace` returns just the projects in one workspace (same shape as `listProjects`) and is empty for an unknown `workspaceID`. `listProjects` still returns every project across all workspaces in one call.

`getWorkspace` returns the workspace for the project's **active** worktree; select the worktree first with `selectWorktree`. It returns `404` when the project has no active workspace. The returned tree includes all top-level and split-child tabs without exposing their internal ownership relationship. `selectTab` accepts either kind; selecting a child also activates its owning top-level tab. `createTab` returns `tab` (the newly active top-level tab) or `500` if creation fails.

## Terminal control

| Method | Parameters | Result |
| --- | --- | --- |
| `takeOverPane` | `paneID`, `cols`, `rows` | `ok` |
| `releasePane` | `paneID` | `ok` |
| `setClientTheme` | `theme?` | `ok` |
| `terminalInput` | `paneID`, `bytes` | `ok` |
| `terminalResize` | `paneID`, `cols`, `rows` | `ok` |
| `terminalScroll` | `paneID`, `deltaX`, `deltaY`, `precise` | `ok` |
| `getTerminalContent` | `paneID` | `terminalCells` |

Notes:

- Terminal control is **ownership-based**. Call `takeOverPane` before sending input, resize, or scroll; `releasePane` returns control to the Mac. Input/resize/scroll from a client that does **not** own the pane are silently dropped (still answered `ok`).
- `takeOverPane` immediately pushes a [`terminalSnapshot`](events.md) event to the calling client, then streams [`terminalOutput`](events.md) until released.
- `terminalInput.bytes` is base64-encoded raw bytes delivered verbatim to the PTY. The client encodes escape sequences, control codes, and mouse reports itself. `terminalInput` is fire-and-forget — the server does **not** send a response for it.
- `getTerminalContent` is a one-shot pull that returns the rendered grid as [`terminalCells`](data-objects.md#terminal-cells). New clients should instead render the pane with their own VT emulator fed by the `terminalOutput` stream; use the pull only for an initial paint or debugging. It returns `404` if the pane has no live surface.
- `setClientTheme` recolors the live Ghostty surfaces of the panes this client **owns** — both currently owned panes and any taken over afterward — with the client's [`clientTheme`](data-objects.md#client-theme). It is optional: clients that never send it keep the Mac theme. Colors revert to the Mac theme when the pane is released or the client disconnects. Send `theme: null` to clear and revert immediately. The theme can also be supplied at [pairing](pairing.md) time.

## Notifications & visual data

| Method | Parameters | Result |
| --- | --- | --- |
| `getProjectLogo` | `projectID` | `projectLogo` |
| `listNotifications` | none | `notifications` |
| `markNotificationRead` | `notificationID` | `ok` |
| `subscribe` | `events` | `ok` |
| `unsubscribe` | `events` | `ok` |

`getProjectLogo` returns `404` when the project has no logo. `subscribe` / `unsubscribe` are accepted but are **no-ops** — the server performs no event filtering, so every authenticated client receives every broadcast event regardless of what it subscribed to.

## Extensions

| Method | Parameters | Result |
| --- | --- | --- |
| `extensionRequest` | `extension`, `action`, `payload` | `extensionResult` |

`extensionRequest` proxies a call to an installed extension that serves the named `action`. Send `payload: null` when there is no argument. `extensionResult.payload` is arbitrary JSON. The desktop resolves the handler, prompts the user for consent, runs it in the extension's background script, and returns its value.

| Code | Meaning |
| --- | --- |
| `404` | Unknown extension, or the action is not declared in `remoteMethods`. |
| `403` | Extension lacks `remote:serve`, or the user denied consent. |
| `503` | Extension is installed but not running / has no live background script. |
| `502` | The handler threw, rejected, or is not registered. |
| `504` | The handler did not reply in time. |

See [extension remote methods](../extensions/remote-methods.md).

## Git & worktrees

| Method | Parameters | Result |
| --- | --- | --- |
| `getVCSStatus` | `projectID` | `vcsStatus` |
| `vcsRefresh` | `projectID` | `vcsStatus` |
| `vcsCommit` | `projectID`, `message`, `stageAll` | `ok` |
| `vcsPush` | `projectID` | `ok` |
| `vcsPull` | `projectID` | `ok` |
| `vcsStageFiles` | `projectID`, `paths` | `ok` |
| `vcsUnstageFiles` | `projectID`, `paths` | `ok` |
| `vcsDiscardFiles` | `projectID`, `paths`, `untrackedPaths` | `ok` |
| `vcsGetDiff` | `projectID`, `filePath`, `forceFull` | `vcsDiff` |
| `vcsListBranches` | `projectID` | `vcsBranches` |
| `vcsSwitchBranch` | `projectID`, `branch` | `ok` |
| `vcsCreateBranch` | `projectID`, `name` | `ok` |
| `vcsCreatePR` | `projectID`, `title`, `body`, `baseBranch?`, `draft` | `vcsPRCreated` |
| `vcsMergePullRequest` | `projectID`, `number`, `method`, `deleteBranch` | `ok` |
| `vcsAddWorktree` | `projectID`, `name`, `branch`, `createBranch`, `baseBranch?` | `worktrees` |
| `vcsRemoveWorktree` | `projectID`, `worktreeID` | `ok` |

Enums & defaults:

- `vcsCommit.stageAll` and `vcsGetDiff.forceFull` are required booleans. Send `false` for the default behavior; `forceFull: false` caps the diff at ~20k lines and sets `truncated`.
- `vcsCreatePR.baseBranch` is optional; when omitted the repo's default branch is used. The result `vcsPRCreated` is `{ "url": string, "number": int }`.
- `vcsMergePullRequest.method` is `merge`, `squash`, or `rebase`; `number` is the PR number.
- `vcsAddWorktree.baseBranch` is only honored when `createBranch` is `true`. The result `worktrees` is an array containing the single new worktree.

`getVCSStatus` and `vcsListBranches` query git for the active worktree. Pull-request metadata may use cached provider data unless `vcsRefresh` forces a fresh refresh. Any VCS method that fails returns `500` with the underlying git error message.

Result shapes: [`vcsStatus`](#vcsstatus-shape) and [`vcsBranches`](#vcsbranches-shape) below; [`vcsDiff`](#vcsdiff-shape) below.

### `vcsStatus` shape

```json
{
  "branch": "main",
  "aheadCount": 1,
  "behindCount": 0,
  "hasUpstream": true,
  "stagedFiles": [{ "path": "a.swift", "status": "modified", "isUntracked": false }],
  "changedFiles": [{ "path": "b.swift", "status": "added", "isUntracked": false }],
  "defaultBranch": "main",
  "pullRequest": {
    "url": "https://github.com/o/r/pull/1",
    "number": 1,
    "state": "OPEN",
    "isDraft": false,
    "baseBranch": "main",
    "mergeable": true,
    "mergeStateStatus": "CLEAN",
    "checks": { "status": "success", "passing": 3, "failing": 0, "pending": 0, "total": 3 }
  }
}
```

`defaultBranch`, `pullRequest`, and `pullRequest.mergeable` are optional. `status` is one of `added`, `modified`, `deleted`, `renamed`, `copied`, `untracked`, `unmerged`.

### `vcsBranches` shape

```json
{ "current": "main", "locals": ["main", "feature"], "defaultBranch": "main" }
```

### `vcsDiff` shape

```json
{
  "filePath": "a.swift",
  "rows": [
    { "kind": "addition", "oldLineNumber": null, "newLineNumber": 12, "oldText": null, "newText": "let x = 1", "text": "let x = 1" }
  ],
  "additions": 1,
  "deletions": 0,
  "truncated": false,
  "isBinary": false
}
```

`kind` is `hunk`, `context`, `addition`, `deletion`, or `collapsed`. Binary files return `isBinary: true` with no rows.

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
