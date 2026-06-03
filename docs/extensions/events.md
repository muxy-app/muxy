# Events

Events let an extension react to what's happening in the workspace — a pane opening, a project switch, one of its own palette commands firing. You subscribe to them by name and get a callback when they occur.

Subscribe from your `background.js`:

```js
muxy.events.subscribe('pane.created', (payload) => {
  console.log('new pane', payload.paneID);
});
```

In a tab/panel/popover page, the same API is on the bridge as `window.muxy.events.subscribe(...)`. The handler receives the payload as a plain object; Muxy handles the host process, identity, and transport for you.

Events originate in the main process from `ExtensionEventEmitter`, which diffs workspace state and fans matching events out to subscribed extensions.

## Subscribing

- **Workspace events** (`pane.*`, `tab.*`, `project.*`, `worktree.*`, `notification.posted`, `file.changed`) must be listed in your manifest `events` array before you can subscribe. Subscribing to anything not declared is rejected.
- **Command events** (`command.<id>`) are auto-allowed: declaring a command in `manifest.commands` is implicit consent to receive its trigger, so you do not add it to `events`.

```json
{
  "events": ["pane.created", "project.switched"]
}
```

When an extension is reloaded or disabled, its subscriptions are dropped and re-filtered against the new manifest.

## Available events

| Event | Payload keys | Allowed by |
| --- | --- | --- |
| `pane.created` | `paneID` | `events: ["pane.created"]` |
| `pane.closed` | `paneID` | `events: ["pane.closed"]` |
| `pane.focused` | `projectID`, `worktreeID`, `areaID`, `tabID` | `events: ["pane.focused"]` |
| `tab.created` | `tabID` | `events: ["tab.created"]` |
| `tab.focused` | `areaID`, `tabID` | `events: ["tab.focused"]` |
| `project.switched` | `projectID` | `events: ["project.switched"]` |
| `worktree.switched` | `projectID`, `worktreeID` | `events: ["worktree.switched"]` |
| `notification.posted` | `paneID`, `projectID`, `tabID`, `title` | `events: ["notification.posted"]` |
| `file.changed` | `path`, `projectPath` | `events: ["file.changed"]` |
| `command.<id>` | `command`, `extension` | Auto-allowed when `commands[].id == <id>` |

`file.changed` fires for files under the active project/worktree root. It is debounced (~0.3s) and skips Git-internal noise (`.git/` lock files and directories); one event is delivered per changed `path`, with `projectPath` set to the watched root. Pair it with [`muxy.files`](files.md) to build a reactive file tree.

See [Permissions](permissions.md) for how `events` fits the manifest, and [Palette Commands](palette-commands.md) for `command.<id>`.
