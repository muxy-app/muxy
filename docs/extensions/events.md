# Events

Events let an extension react to what's happening in the workspace — a pane opening, a project switch, one of its own palette commands firing. They also provide an extension-local channel so a tab, panel, or popover can talk to its own `background.js`, and the background script can send updates back to open webviews.

Subscribe from your `background.js`:

```js
muxy.events.subscribe('pane.created', (payload) => {
  console.log('new pane', payload.paneID);
});
```

In a tab/panel/popover page, the same API is on the bridge as `window.muxy.events.subscribe(...)`. The handler receives the payload as a plain object; Muxy handles the host process, identity, and transport for you.

`muxy.events` exists only in `background.js` and in webview pages. It is **not** available inside [`runScript`](scripts.md) palette-command scripts — those run in a short-lived in-process context with no event channel.

Workspace events originate in the main process from `ExtensionEventEmitter`, which diffs workspace state and fans matching events out to subscribed extensions.

Extension-local events use the reserved `extension.` prefix and stay inside one extension. They are not listed in the manifest, need no permission, and are only delivered between the extension's own webviews and its own background script.

```js
// panel.js
muxy.events.subscribe('extension.refresh.result', (payload) => {
  render(payload);
});
await muxy.events.emit('extension.refresh.request', { source: muxy.tabInstanceID });
```

```js
// background.js
muxy.events.subscribe('extension.refresh.request', async () => {
  const status = await muxy.git.status();
  await muxy.events.emit('extension.refresh.result', { status });
});
```

## Subscribing

- **Workspace events** (`pane.*`, `tab.*`, `panel.*`, `popover.*`, `project.*`, `projects.changed`, `worktree.*`, `notification.posted`, `agent.status`, `file.changed`) must be listed in your manifest `events` array before you can subscribe. Subscribing to anything not declared is rejected.
- **Permission-gated events** also require their read permission to subscribe: `projects.changed` needs `projects:read`, `agent.status` needs `agents:read`, `file.changed` needs `files:read`. Declaring the event without the permission is rejected.
- **Command events** (`command.<id>`) are auto-allowed: declaring a command in `manifest.commands` is implicit consent to receive its trigger, so you do not add it to `events`.
- **Extension-local events** (`extension.*`) are auto-allowed for the same extension. They are not workspace events, do not appear in `events`, and cannot cross extension boundaries.

```json
{
  "events": ["pane.created", "project.switched"]
}
```

When an extension is reloaded or disabled, its subscriptions are dropped and re-filtered against the new manifest.

`muxy.events.subscribe(name, handler)` returns an unsubscribe function on webviews and background scripts. `muxy.events.emit(name, payload?)` accepts only `extension.*` names. Payloads must be JSON-serializable and are capped at 64 KiB. A webview emit is relayed through the extension's `background.js`, so it rejects when no background script is running.

## Available events

| Event | Payload keys | Allowed by |
| --- | --- | --- |
| `pane.created` | `paneID`, `tabID`, `kind`, `projectID`, `worktreeID`, `areaID`, `title`, `projectPath`, and optionally `cwd`, `extensionID`, `tabTypeID` | `events: ["pane.created"]` |
| `pane.closed` | `paneID`, `tabID`, `kind`, `projectID`, `worktreeID`, `areaID`, `title`, `projectPath`, and optionally `cwd`, `extensionID`, `tabTypeID` | `events: ["pane.closed"]` |
| `pane.focused` | `projectID`, `worktreeID`, `areaID`, `tabID` | `events: ["pane.focused"]` |
| `tab.created` | `tabID`, `kind`, `projectID`, `worktreeID`, `areaID`, `title`, `projectPath`, and optionally `paneID`, `cwd`, `extensionID`, `tabTypeID`, `data` | `events: ["tab.created"]` |
| `tab.updated` | `tabID`, `kind`, `projectID`, `worktreeID`, `areaID`, `title`, `projectPath`, and optionally `paneID`, `cwd`, `extensionID`, `tabTypeID`, `data` | `events: ["tab.updated"]` |
| `tab.closed` | `tabID`, `kind`, `projectID`, `worktreeID`, `areaID`, `title`, `projectPath`, and optionally `paneID`, `cwd`, `extensionID`, `tabTypeID`, `data` | `events: ["tab.closed"]` |
| `tab.focused` | `areaID`, `tabID` | `events: ["tab.focused"]` |
| `panel.opened` | `extensionID`, `panelID` | `events: ["panel.opened"]` |
| `panel.closed` | `extensionID`, `panelID` | `events: ["panel.closed"]` |
| `popover.opened` | `extensionID`, `popoverID` | `events: ["popover.opened"]` |
| `popover.closed` | `extensionID`, `popoverID` | `events: ["popover.closed"]` |
| `project.switched` | `projectID` | `events: ["project.switched"]` |
| `projects.changed` | _(none)_ | `events: ["projects.changed"]` + `projects:read` |
| `worktree.switched` | `projectID`, `worktreeID` | `events: ["worktree.switched"]` |
| `worktree.headChanged` | `projectID`, `worktreeID`, `branch`, `path` | `events: ["worktree.headChanged"]` + `worktrees:read` |
| `notification.posted` | `paneID`, `projectID`, `worktreeID`, `worktreePath`, `tabID`, `source`, `title`, `body` | `events: ["notification.posted"]` |
| `agent.status` | `worktreeID`, `projectID`, `paneID`, `providerID`, `status` | `events: ["agent.status"]` + `agents:read` |
| `file.changed` | `path`, `projectPath` | `events: ["file.changed"]` + `files:read` |
| `command.<id>` | `command`, `extension` | Auto-allowed when `commands[].id == <id>` |
| `extension.<name>` | JSON payload from emitter | Auto-allowed same-extension local event |

`projects.changed` fires whenever the project list changes — a project is added, renamed, recolored, re-iconed, reordered, or removed — whether the change came from Muxy's own UI or from an extension verb. It carries no payload; webviews can call [`muxy.projects.list()`](permissions.md) to refetch the current list, while background scripts should notify a webview through an `extension.*` event.

`agent.status` reports an AI coding agent's lifecycle per worktree, driven by the provider's hooks: `working` when a prompt is submitted or the agent runs a tool, `waiting` when the agent needs attention, `idle` when it stops. `providerID` identifies the agent (e.g. `claude`). When a worktree holds several agent panes, the reported status is the most active one (`working` > `waiting` > `idle`) and `paneID` points to the pane that owns it. It fires only when the worktree status changes, and turns `idle` once the last agent pane in the worktree closes. Pair it with [`muxy.agents.list()`](permissions.md) to hydrate current statuses on load.

Which states a provider reports depends on the hooks its CLI exposes:

| Provider (`providerID`) | `working` | `waiting` | `idle` |
| --- | --- | --- | --- |
| Claude Code (`claude`) | ✓ | ✓ | ✓ |
| Droid (`droid`) | ✓ | ✓ | ✓ |
| Grok (`grok`) | ✓ | ✓ | ✓ |
| OpenCode (`opencode`) | ✓ | ✓ | ✓ |
| Pi (`pi`) | ✓ | — | ✓ |
| Cursor (`cursor`) | — | ✓ | ✓ |
| Codex (`codex`) | — | — | ✓ |

A `—` means the CLI's hooks have no event for that transition, so the provider never emits that state.

`worktree.headChanged` fires when a worktree's checked-out branch changes — e.g. a `git checkout` in a terminal — detected by watching `.git/HEAD` (no polling). The payload carries the new `branch` and the worktree `path`. Pair it with [`muxy.git.worktrees()`](git.md) to refresh a worktree tree reactively instead of polling.

`file.changed` fires for files under the active project/worktree root. It is debounced (~0.3s) and skips Git-internal noise (`.git/` lock files and directories); one event is delivered per changed `path`, with `projectPath` set to the watched root. Pair it with [`muxy.files`](files.md) to build a reactive file tree.

The enriched `tab.created` / `tab.updated` / `tab.closed` / `pane.created` / `pane.closed` payloads carry the full context of the surface — `kind` (`"terminal"` or `"extensionWebView"`), `projectID`, `worktreeID`, `areaID`, `title`, `projectPath`, and where relevant `cwd`, `extensionID`, and `tabTypeID` — so an extension can recreate a tab without a separate lookup. Keys are omitted when nil (e.g. `cwd` only appears for terminal surfaces, `extensionID`/`tabTypeID` only for extension webviews).

`tab.updated` fires when an existing tab's restore-relevant state changes — its `title`, `cwd`, or extension `data`. It is coalesced and title changes are debounced (~0.5s). The `tab.*` payloads also carry an optional `data` key for extension-webview tabs: a JSON-encoded string of the tab's `data` blob (the same object passed as `extension.data` to [`muxy.tabs.open`](tabs.md)), omitted for terminal tabs and when there is no data. `JSON.parse` it and pass it back to `tabs.open` to recreate the tab — this is the basis for extension-driven session restore, where an extension records `tab.created`/`tab.updated`/`tab.closed` for every tab, terminal and extension alike, and replays them later.

`tab.closed`, `panel.closed`, and `popover.closed` fire **after** the surface is actually removed. To *prevent* a close (e.g. an unsaved editor), don't use these observation events — use [Lifecycle](lifecycle.md), which asks your surface for an allow/prevent verdict *before* it closes.

See [Permissions](permissions.md) for how `events` fits the manifest, [Lifecycle](lifecycle.md) for intercepting closes, and [Palette Commands](palette-commands.md) for `command.<id>`.
