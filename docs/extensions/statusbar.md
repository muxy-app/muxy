# Status Bar Items

A status bar item is an icon (with optional text) Muxy adds to either side of the footer status bar — the row that shows the active path, repository controls, extension output, resource usage, and rich-input controls. Clicking it runs one of the extension's declared [commands](palette-commands.md).

```json
{
  "commands": [
    { "id": "show-builds", "title": "Builds", "action": { "kind": "openTab", "tabType": "builds" } }
  ],
  "statusBarItems": [
    {
      "id": "build",
      "icon": { "symbol": "hammer.fill" },
      "text": "0",
      "tooltip": "Show recent builds",
      "side": "right",
      "command": "show-builds"
    }
  ]
}
```

## Fields

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `id` | string | yes | Unique within the extension. |
| `icon` | object | yes | `{ "symbol": "<sf-symbol>" }` or `{ "svg": "<path>" }`. See [Icons](manifest.md#icons). |
| `text` | string | no | Static text shown next to the icon. Can be replaced at runtime — see below. |
| `tooltip` | string | no | Hover tooltip / accessibility label. Defaults to the `id`. |
| `side` | string | yes | `left` or `right`. Groups with the built-in entries on that side. |
| `command` | string | yes | Must reference a declared `commands[].id`. |
| `visible` | boolean | no | Whether the item shows on load. Defaults to `true`. Set `false` to start hidden and reveal it later with `muxy.statusbar.show`. |

## Updating an item at runtime

The icon, text, and visibility can change while the extension runs — from `background.js`, any tab/panel/popover page, or a [`runScript`](scripts.md) command — with `muxy.statusbar.set`:

```js
muxy.statusbar.set({ id: "build", text: "42" });
muxy.statusbar.set({ id: "build", icon: { symbol: "checkmark.circle.fill" }, text: "✓" });
muxy.statusbar.set({ id: "build", text: null }); // clear text back to the manifest value
muxy.statusbar.set({ id: "build", visible: false });
muxy.statusbar.show("build"); // sugar for { visible: true }
muxy.statusbar.hide("build"); // sugar for { visible: false }
```

| Field | Type | Notes |
| --- | --- | --- |
| `id` | string | Must reference a declared `statusBarItems[].id`. |
| `icon` | string \| object | New icon: `"<sf-symbol>"`, `{ symbol }`, or `{ svg }` (the SVG must be a file bundled with the extension). Omit to leave the icon unchanged. |
| `text` | string \| null | New text. `null` or `""` clears the override back to the manifest value. Omit to leave the text unchanged. |
| `visible` | boolean | Show or hide the item. Omit to leave visibility unchanged. |

Decide visibility at runtime: declare the item with `"visible": false` so it stays hidden until your `background.js` calls `muxy.statusbar.show(id)`, then `muxy.statusbar.hide(id)` when it no longer applies.

Needs `panels:write`. Overrides are in-memory for the session; disabling or reloading the extension restores the manifest values. Throws on an unknown `id`.

### Socket alternative (CLI)

The text can also be set over the **socket** with `extension.statusbar.set|<itemID>[|<text>]`, used by the `muxy` CLI and advanced integrations. Muxy handles the identity handshake; omitting the text clears the override.

| Response | Meaning |
| --- | --- |
| `ok` | Text updated (or cleared, when no text is given). |
| `error:identify required` | The connection has not been identified yet. |
| `error:unknown status bar item '<id>'` | The id is not declared in `statusBarItems`. |
