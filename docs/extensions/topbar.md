# Topbar Items

Extensions can attach icons to the right-hand side of the tab strip — the same row that holds the VCS, file-diff, and file-tree buttons. Clicking an icon triggers one of the extension's declared palette commands.

```json
{
  "commands": [
    { "id": "open-pr", "title": "Open PR…", "action": { "kind": "openTab", "tabType": "pr-viewer" } }
  ],
  "topbarItems": [
    {
      "id": "pr",
      "icon": { "symbol": "arrow.triangle.pull" },
      "tooltip": "Open Pull Request",
      "command": "open-pr"
    }
  ]
}
```

## Fields

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `id` | string | yes | Unique within the extension. |
| `icon` | object | yes | `{ "symbol": "<sf-symbol>" }` or `{ "svg": "<path>" }`. See [Icons](manifest.md#icons). A bare string is treated as a symbol. |
| `tooltip` | string | no | Hover tooltip and accessibility label. Defaults to the id. |
| `command` | string | yes | Must reference a declared `commands[].id`. |

## Behavior

Clicking a topbar icon dispatches the referenced command through the same path as the command palette — including the command's `action` (`event`, `openTab`, or `runScript`). Permissions required by the resulting action still apply (e.g. `commands:run-script` for a `runScript` action).

Disabled extensions contribute no topbar items. Items disappear immediately when the extension is toggled off.

## Limits

- Topbar icons render in the order extensions are loaded.
- An item whose `command` references an unknown id fails the manifest load — fix the reference before reloading.
- SVG icons must live inside the extension directory and exist at load time.
