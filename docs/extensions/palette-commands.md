# Palette Commands

Extensions can declare commands that appear in Muxy's command palette. Picking a command either fires a `command.<id>` event back to your extension or runs a built-in action (open a tab, toggle a panel, run a script).

```json
{
  "commands": [
    { "id": "ping", "title": "Hello: Ping", "subtitle": "Demo command" },
    {
      "id": "open-pr",
      "title": "Open PR…",
      "action": { "kind": "openTab", "tabType": "pr-viewer" }
    }
  ]
}
```

## Fields

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `id` | string | yes | Stable per extension. Forms the event name `command.<id>`. |
| `title` | string | yes | The palette row title. |
| `subtitle` | string | no | Dimmer second line. Defaults to the extension's display name. |
| `action` | object | no | What happens when picked. Defaults to `{ "kind": "event" }`. |
| `defaultShortcut` | string | no | A keyboard shortcut that runs the command, e.g. `"cmd+shift+e"`. See below. |

## Keyboard shortcuts

A command may declare a `defaultShortcut` (e.g. `"cmd+ctrl+p"`). It must include
at least one of `cmd`, `ctrl`, or `opt`; bare keys are ignored. On load Muxy
auto-assigns it when the combo is free; if it is already taken (by an app
shortcut, a custom command, or another extension), the command registers
**unassigned**. The first extension to claim a combo keeps it. Users view and
rebind these under **Settings → Keyboard Shortcuts → App Shortcuts**, grouped by
extension name. Pressing the shortcut runs the command's `action`, exactly as
picking it from the palette does.

## Runtime shortcuts

`defaultShortcut` is **static** — declared in the manifest and bound at load. To bind a shortcut **at runtime** (e.g. a combo configured in the extension's own settings), use `muxy.shortcuts`. A runtime shortcut fires the same `command.<id>` event, so you subscribe to it the same way; it needs no matching command in `commands` (the `id` is free-form). Best called from `background.js`, which re-registers on each launch — runtime shortcuts are not persisted.

```js
const { ok, conflict } = await muxy.shortcuts.register({ id: 'toggle', combo: 'cmd+b' });
if (!ok) console.warn('shortcut not bound:', conflict);

muxy.events.subscribe('command.toggle', () => { /* react */ });

await muxy.shortcuts.unregister('toggle');
const bound = await muxy.shortcuts.list(); // [{ id, combo, source: 'manifest' | 'runtime' }]
```

| Method | Returns | Notes |
| --- | --- | --- |
| `register({ id, combo })` | `{ ok, conflict? }` | `combo` is parsed like `defaultShortcut` (must include `cmd`/`ctrl`/`opt`). `ok` is `false` with a `conflict` message when the combo is already taken; re-registering an `id` updates its combo. |
| `unregister(id)` | — | No-op if the id is not registered. |
| `list()` | array of `{ id, combo, source }` | The calling extension's manifest and runtime shortcuts. |

Registering/unregistering needs the `shortcuts:register` permission. An `id` that matches one of your manifest `commands` is rejected — bind those via `defaultShortcut`. Runtime shortcuts are cleared when the extension is disabled.

## Actions

| Kind | Behavior | Extra fields |
| --- | --- | --- |
| `event` | Fires `command.<id>` to your extension. Default if `action` is omitted. | — |
| `openTab` | Opens an extension webview tab of the named type. | `tabType` (required, must reference a declared [tab type](tabs.md)); `data` (optional JSON merged into `window.muxy.data`). |
| `togglePanel` | Toggles an extension [panel](panels.md) open/closed. | `panel` (required, must reference a declared panel id). |
| `openPopover` | Toggles an extension [popover](popovers.md) anchored to its topbar/status-bar item. | `popover` (required, must reference a declared popover id). |
| `runScript` | Runs a script in an in-process JavaScriptCore context (no DOM). It exposes a **synchronous** `muxy.*` API: `tabs`, `panes`, `projects`, `worktrees`, `browser`, `agents`, `files`, `git`, `exec`, `dialog`, `modal`, `topbar`, `statusbar`, `notifications`/`toast`. It has no `events`, `remote`, `http`, `panels`, or `popover`. See [Scripts](scripts.md). Requires `commands:run-script`. | `script` (required, relative path within the extension directory). |

## How it surfaces

Commands appear in the **Custom Commands** scope of the omnibox (default `⌘⇧P`), under an **Extension Commands** section, searchable by extension name, title, and subtitle.

## Reacting to a command

For the default `event` action, subscribe to your own command event in `background.js`. The command id auto-allows its `command.<id>` event, so you do **not** add it to the manifest `events` array.

```js
muxy.events.subscribe('command.ping', ({ command, extension }) => {
  // react, e.g. post a notification
});
```

## Permissions

Registering a command needs no permission. Reacting to one requires whatever permission the reaction needs (e.g. `notifications:write` to post a toast, `panes:write` to open a split). The `runScript` action additionally requires `commands:run-script` (and any shell call inside it requires `commands:exec`).

## Limits and gotchas

- Disabled extensions contribute no commands; they leave the palette the moment the extension is toggled off in Settings.
- Titles are not deduplicated across extensions. Prefix yours (`MyExt: Build`) to disambiguate.
