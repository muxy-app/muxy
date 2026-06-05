# Events

Events let an extension react to what's happening in the workspace — a pane opening, a project switch, one of its own palette commands firing. They also provide an extension-local channel so a tab, panel, or popover can talk to its own `background.js`, and the background script can send updates back to open webviews.

Subscribe from your `background.js`:

```js
muxy.events.subscribe('pane.created', (payload) => {
  console.log('new pane', payload.paneID);
});
```

In a tab/panel/popover page, the same API is on the bridge as `window.muxy.events.subscribe(...)`. The handler receives the payload as a plain object; Muxy handles the host process, identity, and transport for you.

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

- **Workspace events** (`pane.*`, `tab.*`, `project.*`, `worktree.*`, `notification.posted`, `file.changed`) must be listed in your manifest `events` array before you can subscribe. Subscribing to anything not declared is rejected.
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
| `extension.<name>` | JSON payload from emitter | Auto-allowed same-extension local event |

`file.changed` fires for files under the active project/worktree root. It is debounced (~0.3s) and skips Git-internal noise (`.git/` lock files and directories); one event is delivered per changed `path`, with `projectPath` set to the watched root. Pair it with [`muxy.files`](files.md) to build a reactive file tree.

See [Permissions](permissions.md) for how `events` fits the manifest, and [Palette Commands](palette-commands.md) for `command.<id>`.
