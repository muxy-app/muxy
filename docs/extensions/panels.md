# Extension Panels

Panels are dockable or floating webviews that live alongside Muxy's built-in panels (Source Control, Files, Rich Input). They render the same way as [tabs](tabs.md) — each panel is its own `WKWebView` with the injected `window.muxy` API — but instead of a tab they occupy a docked slot or float over the workspace.

All panels in Muxy, built-in and extension, obey the same placement rules:

- At most **one pinned panel per position** (right or bottom). Pinning another panel to a position unpins the current one.
- At most **one floating panel per position**. Opening a floating panel where one is already floating closes the existing one.

## Declaring a panel

```json
{
  "name": "review-tools",
  "version": "0.1.0",
  "permissions": ["panels:write"],
  "panels": [
    {
      "id": "review",
      "title": "Review",
      "icon": "checklist",
      "entry": "panels/review.html",
      "position": "right",
      "mode": "floating",
      "hiddenControls": ["position"]
    }
  ],
  "commands": [
    {
      "id": "open-review",
      "title": "Open Review Panel",
      "action": { "kind": "togglePanel", "panel": "review" }
    }
  ]
}
```

### Fields

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `id` | string | yes | Stable per extension. Referenced from commands and from `muxy.panels.*`. |
| `entry` | string | yes | Path relative to the extension directory. Must resolve inside the directory (no `..` traversal). |
| `title` | string | no | Shown in the panel header. Omit to hide the title. |
| `icon` | string \| object | no | SF Symbol name or `{ "svg": "assets/icon.svg" }`. Shown in the panel header. |
| `position` | string | no | `right` or `bottom`. Defaults to `right`. |
| `mode` | string | no | `floating` or `pinned`. Defaults to `floating`. |
| `hiddenControls` | string[] | no | Header controls to hide: any of `close`, `pin`, `position`. Defaults to none hidden. |
| `defaultData` | object | no | JSON payload merged into `window.muxy.data` when no explicit data is passed. |

The loader validates that `entry` exists inside the extension directory, that panel ids are unique, and that `togglePanel` commands reference a declared panel id.

## Opening and closing

A `togglePanel` command toggles the panel from the palette, a topbar button, or a status bar item.

From a webview (a tab or another panel), the `panels:write` permission unlocks:

```ts
window.muxy.panels.open(panelID, data?): Promise<void>;
window.muxy.panels.toggle(panelID, data?): Promise<void>;
window.muxy.panels.close(panelID): Promise<void>;
```

From an entrypoint subprocess over the socket:

```
panel.open|<panelID>[|<json-data>]
panel.toggle|<panelID>[|<json-data>]
panel.close|<panelID>
```

`data` overrides the panel's `defaultData` for that instance and is exposed to the page as `window.muxy.data`.

## Header controls

The panel header is owned by the host. It shows the optional icon and title on the left, and on the right — unless hidden via `hiddenControls` — a position toggle (right ⇄ bottom), a pin toggle (float ⇄ dock), and a close button. The webview content fills the rest of the panel.

Panels close automatically when the extension is disabled or stopped.
