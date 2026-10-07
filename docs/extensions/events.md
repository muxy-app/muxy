# Events

Pages and background scripts can subscribe to workspace events. List every
workspace event you use in `muxy.events`; some also need a permission.

```json
"events": ["tab.focused", "agent.status"],
"permissions": ["agents:read"]
```

```js
const unsubscribe = muxy.events.subscribe("agent.status", (event) => {
  console.log(event.providerID, event.status);
});
```

Every payload value is a string.

## Workspace events

| Event | Payload | Needs |
| --- | --- | --- |
| `tab.created`, `tab.closed`, `tab.updated` | Tab context, below | |
| `tab.focused` | `areaID`, `tabID` | |
| `pane.created`, `pane.closed` | Tab context plus `paneID` (terminal panes) | |
| `pane.focused` | `projectID`, `worktreeID`, `areaID`, `tabID` | |
| `project.switched` | `projectID` | |
| `worktree.switched` | `projectID`, `worktreeID` | |
| `projects.changed` | none | `projects:read` |
| `worktree.headChanged` | `projectID`, `worktreeID`, `branch`, `path` | `worktrees:read` |
| `agent.status` | `projectID`, `worktreeID`, `paneID`, `providerID`, `status` (`working`, `waiting`, `idle`) | `agents:read` |
| `file.changed` | `path`, `projectPath` | `files:read` |
| `notification.posted` | `paneID`, `projectID`, `worktreeID`, `worktreePath`, `tabID`, `source`, `title`, `body` | |
| `panel.opened`, `panel.closed` | `extensionID`, `panelID` | |
| `popover.opened`, `popover.closed` | `extensionID`, `popoverID` | |
| `modal.opened`, `modal.closed` | `extensionID`, `modalID` | |

**Tab context:** `tabID`, `projectID`, `worktreeID`, `areaID`, `title`,
`projectPath`, and `kind` (`terminal` or `extensionWebView`). Terminal tabs add
`paneID` and `cwd`. Extension tabs add `extensionID`, `tabTypeID`,
`tabInstanceID`, and `data` as a JSON string.

- `projectID` is the top-level project. `worktreeID` is the project or
  worktree the tab belongs to.
- `file.changed` covers the current project and projects with the extension's
  tabs open.
- `agent.status` reports the busiest agent per worktree; `providerID` is
  `claude`, `codex`, `opencode`, `cursor`, `copilot`, `droid`, `pi`, `grok`,
  `kiro`, `xal`, `antigravity`, or `other`.

## Command events

A command with the `event` action sends `command.<id>` to the extension. No
declaration needed.

## Extension messages

An extension's pages and background script can message each other with
`extension.*` events. No declaration needed; they never leave the extension.

```js
// page
await muxy.events.emit("extension.refresh", { reason: "button" });

// background
muxy.events.subscribe("extension.refresh", (payload) => { /* ... */ });
```

A page's message needs the background script running. Payloads are JSON, up to
64 KB.

## Background scripts

Set `muxy.background` to a script path to run code while the extension is
enabled. It starts with the extension, restarts when the extension updates,
and stops when it is disabled or reloaded. Use it to react to events, keep
state, or update [bar items](bar-items.md). Most extensions don't need one.

```js
// background.js
muxy.events.subscribe("tab.focused", () => {
  muxy.statusbar.set({ id: "count", text: String(muxy.agents.list().length) });
});
```

Background scripts run in-process on their own thread. Errors and `console`
output go to the extension's log.
