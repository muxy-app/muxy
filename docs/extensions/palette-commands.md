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

## Actions

| Kind | Behavior | Extra fields |
| --- | --- | --- |
| `event` | Fires `command.<id>` to your extension. Default if `action` is omitted. | — |
| `openTab` | Opens an extension webview tab of the named type. | `tabType` (required, must reference a declared [tab type](tabs.md)); `data` (optional JSON merged into `window.muxy.data`). |
| `togglePanel` | Toggles an extension [panel](panels.md) open/closed. | `panel` (required, must reference a declared panel id). |
| `openPopover` | Toggles an extension [popover](popovers.md) anchored to its topbar/status-bar item. | `popover` (required, must reference a declared popover id). |
| `runScript` | Runs a script in an in-process JavaScriptCore context with the same `muxy.*` API as webview tabs (no DOM). See [Scripts](scripts.md). Requires `commands:run-script`. | `script` (required, relative path within the extension directory). |

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

There is no `commands:*` permission. Registering a command is free; reacting to one requires whatever permission the reaction needs (e.g. `notifications:write` to post a toast, `panes:write` to open a split). The `runScript` action additionally requires `commands:run-script`.

## Limits and gotchas

- Disabled extensions contribute no commands; they leave the palette the moment the extension is toggled off in Settings.
- Titles are not deduplicated across extensions. Prefix yours (`MyExt: Build`) to disambiguate.
