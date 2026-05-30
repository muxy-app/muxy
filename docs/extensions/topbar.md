# Topbar Items

A topbar item is an icon Muxy adds to the right-hand cluster of the tab strip — the same row that holds the VCS, file-diff, and file-tree buttons. Clicking it runs one of the extension's declared [commands](palette-commands.md).

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
| `icon` | object | yes | `{ "symbol": "<sf-symbol>" }` or `{ "svg": "<path>" }`. A bare string is treated as a symbol. See [Icons](manifest.md#icons). |
| `tooltip` | string | no | Hover tooltip and accessibility label. Defaults to the `id`. |
| `command` | string | yes | Must reference a declared `commands[].id`. |

## Behavior

A click dispatches the referenced command through the same path as the command palette, running its `action` (`event`, `openTab`, or `runScript`). The action's permissions still apply — e.g. a `runScript` action needs `commands:run-script`. See [Permissions](permissions.md).

Disabled extensions contribute no topbar items.

## Placement and order

Items sit in the right-hand cluster, just before the built-in **Split / New Tab** group. Among themselves they are ordered by extension directory name, then by their order in the `topbarItems` array.

## Limits

- A `command` that references an unknown id fails the manifest load.
- SVG icons must live inside the extension directory, end in `.svg`, and be at most 256 KiB.
