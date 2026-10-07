# Panels

Panels, popovers, and the sidebar are web pages around the workspace. They all
get the same page API, [data](tabs.md#page-data), and [theme](tabs.md#theme)
as tabs.

## Panels

A panel docks to the right or bottom of the workspace, or floats over it.

```json
"panels": [
  {
    "id": "notes",
    "title": "Notes",
    "icon": "note.text",
    "entry": "notes/index.html",
    "position": "right",
    "mode": "floating"
  }
]
```

| Field | Notes |
| --- | --- |
| `id`, `entry` | Required. |
| `title`, `icon` | Shown in the panel header. |
| `position` | `right` (default) or `bottom`. |
| `mode` | `floating` (default) or `pinned`. Users can switch, and their choice is kept. |
| `hiddenControls` | Hide header controls: `icon`, `title`, `close`, `pin`, `position`. |
| `headerButtons` | Extra header buttons, shaped like [bar items](bar-items.md). |
| `hideTopbar` | Hide the whole header. |
| `defaultData` | Data used when opened without any. |

- Open one with a `togglePanel` or `openPanel` [command](commands.md), or from
  a page with `muxy.panels.open(id, data)`, `toggle(id, data)`, or
  `close(id)`. `close()` with no ID closes the calling panel. Needs
  `panels:write`.
- Each project remembers its open panel: switching projects closes it, and
  switching back reopens it.
- A floating panel closes when you click elsewhere.

## Popovers

A popover opens under a top bar or status bar item, and closes when you click
outside it.

```json
"popovers": [{ "id": "status", "entry": "status/index.html", "width": 320, "height": 360 }],
"commands": [{ "id": "show-status", "title": "Status", "action": { "kind": "openPopover", "popover": "status" } }],
"statusBarItems": [{ "id": "status", "icon": "bolt", "side": "right", "command": "show-status" }]
```

- Popovers open only from bar items whose command uses `openPopover`. Needs
  `panels:write`.
- `width` and `height` default to 320 × 360. `muxy.popover.resize(w, h)`
  changes them within 80–600 × 80–720.
- `muxy.popover.close()` closes it.

## Sidebar

An extension can provide one full-height page that replaces the built-in
sidebar.

```json
"sidebar": { "id": "main", "title": "My Sidebar", "icon": "sidebar.left", "entry": "sidebar/index.html" }
```

Users turn it on in **Settings → Appearance → Active sidebar**. It can't close
itself.
