# Status Bar Items

A status bar item is an icon (with optional text) Muxy adds to either side of the footer status bar — the row that shows the project path, branch, and rich-input controls. Clicking it runs one of the extension's declared [commands](palette-commands.md).

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

## Updating text at runtime

Item text can be changed at runtime over the **socket** — this is the `extension.statusbar.set` verb, used by the `muxy` CLI and advanced integrations. It is **not** currently exposed as a method on the background `muxy` global, so a `background.js` script cannot set it directly today.

The socket contract is `extension.statusbar.set|<itemID>[|<text>]`. Muxy handles the identity handshake; callers do not write it themselves. Omitting the text argument clears the override back to the manifest value.

| Response | Meaning |
| --- | --- |
| `ok` | Text updated (or cleared, when no text is given). |
| `error:identify required` | The connection has not been identified yet. |
| `error:unknown status bar item '<id>'` | The id is not declared in `statusBarItems`. |

The override is in-memory for the session; disabling or reloading the extension clears it.
