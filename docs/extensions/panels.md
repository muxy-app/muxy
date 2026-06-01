# Extension Panels

A panel is a webview that docks beside the workspace or floats over it, alongside Muxy's built-in panels (Source Control, Files, Rich Input). Each panel is its own `WKWebView` with the injected [`window.muxy`](tabs.md#windowmuxy) bridge, just like a [tab](tabs.md) — it simply occupies a docked or floating slot instead of a tab.

Every panel, built-in or extension, follows the same placement rules per position (`right` or `bottom`):

- **One pinned panel per position.** Pinning another panel to a position unpins the current one.
- **One floating panel per position.** Opening a floating panel where one already floats closes the existing one.

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
    { "id": "open-review", "title": "Open Review Panel", "action": { "kind": "togglePanel", "panel": "review" } }
  ]
}
```

### Fields

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `id` | string | yes | Stable per extension. Referenced from `togglePanel` commands and from `muxy.panels.*`. |
| `entry` | string | yes | HTML path relative to the extension directory. Must resolve inside it (no `..` traversal). |
| `title` | string | no | Shown in the panel header. Omit to hide the title. |
| `icon` | string \| object | no | SF Symbol name, or `{ "svg": "assets/icon.svg" }`. Shown in the header. |
| `position` | string | no | `right` or `bottom`. Defaults to `right`. |
| `mode` | string | no | `floating` or `pinned`. Defaults to `floating`. |
| `hiddenControls` | string[] | no | Header controls to hide: any of `close`, `pin`, `position`. Defaults to none hidden. |
| `hideTopbar` | boolean | no | Hide the entire panel header, including icon, title, and all controls. Your webview fills the whole panel. Defaults to `false`. |
| `defaultData` | object | no | JSON merged into `window.muxy.data` when no explicit data is passed. |

## Header controls

The host owns the panel header: optional icon and title on the left; on the right (unless hidden via `hiddenControls`) a position toggle (right ⇄ bottom), a pin toggle (float ⇄ dock), and a close button. Your webview fills the rest.

Set `hideTopbar: true` to drop the header entirely — no icon, title, or controls — and render the panel as a single edge-to-edge webview. The panel must then provide its own way to close itself (e.g. a `togglePanel` command, or `window.muxy.panels.close`).

## Opening and closing

A `togglePanel` command toggles the panel from the palette, a topbar button, or a status bar item.

From any page (a tab or another panel), with the `panels:write` permission:

```ts
window.muxy.panels.open(panelID, data?): Promise<void>;
window.muxy.panels.toggle(panelID, data?): Promise<void>;
window.muxy.panels.close(panelID): Promise<void>;
```

`data` overrides the panel's `defaultData` for that instance and is exposed to the page as `window.muxy.data`. Opening a panel is a page capability — the background script has no panels API. Panels close automatically when the extension is disabled or stopped.
