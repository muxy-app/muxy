# Status Bar Items

Extensions can place items in either side of the footer status bar — the row that shows the project path, branch, and rich-input controls. Each item has an icon, optional text, and triggers one of the extension's declared palette commands.

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
| `tooltip` | string | no | Hover tooltip / accessibility label. Defaults to the id. |
| `side` | string | yes | `left` or `right`. Items group with the built-in status bar entries on that side. |
| `command` | string | yes | Must reference a declared `commands[].id`. |

## Updating text at runtime

```
identify|<extension-id>|<token>
extension.statusbar.set|<itemID>|<text>
```

`<token>` comes from the `MUXY_EXTENSION_TOKEN` environment variable Muxy injects when it spawns the extension. The connecting process must echo it back; identify is rejected otherwise.

| Response | Meaning |
| --- | --- |
| `ok` | Text updated. To clear back to the manifest value, send `extension.statusbar.set\|<itemID>` (no third argument) or pass an empty text \(`extension.statusbar.set\|<itemID>\|`\). |
| `error:identify required` | Connection has not called `identify` yet. |
| `error:unknown status bar item '<id>'` | The id is not declared in the extension's `statusBarItems`. |

The override lives in-memory for the lifetime of the session. Disabling or reloading the extension clears it.

## Separators

The footer status bar draws a 1-pixel separator between every item on each side, including extension items. A separator is appended after the last left item and prepended before the first right item, so the two groups always have a visible edge against the central spacer.
