# Extension Popovers

A popover is a transient webview anchored to a [topbar](topbar.md) or [status bar](statusbar.md) item. Clicking the item opens it; clicking outside dismisses it. Unlike [panels](panels.md), it never docks and is not persisted — use it for a quick, read-mostly view (a usage meter, status summary, small action list). Each popover is a `WKWebView` with the injected [`window.muxy`](tabs.md#windowmuxy) bridge.

At most **one extension popover is open at a time** — opening another anchor's popover closes the current one.

## Declaring a popover

```json
{
  "name": "ai-usage",
  "version": "0.1.0",
  "permissions": ["panels:write"],
  "popovers": [
    { "id": "usage", "title": "AI Usage", "entry": "index.html", "width": 320, "height": 360 }
  ],
  "commands": [
    { "id": "open-usage", "title": "Open AI Usage", "action": { "kind": "openPopover", "popover": "usage" } }
  ],
  "statusBarItems": [
    { "id": "usage", "icon": "sparkles", "side": "right", "command": "open-usage" }
  ]
}
```

A popover is always reached through a topbar/status bar item whose `command` resolves to an `openPopover` action, and it anchors to that exact item. There is no `open` verb — popovers are user-triggered, and the background script does not drive them.

### Fields

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `id` | string | yes | Stable per extension. Referenced from an `openPopover` command. |
| `entry` | string | yes | HTML path relative to the build output — any layout works (e.g. root `index.html`); not a fixed `popovers/` folder. Must resolve inside the extension directory (no `..` traversal). |
| `title` | string | no | Available to the page; the popover is frameless (no host chrome). |
| `width` | number | no | Initial width in points. Defaults to `320`. |
| `height` | number | no | Initial height in points. Defaults to `360`. |
| `defaultData` | object | no | JSON exposed to the page as `window.muxy.data`. |

## Sizing and closing

From the popover page, with `panels:write`, you can resize the popover to fit its content and close it. Both act on the popover currently open for the calling extension; the host clamps the reported size to a sane range.

```ts
window.muxy.popover.resize(width, height): Promise<void>;
window.muxy.popover.close(): Promise<void>;
```

A common pattern is to report the document size once it has laid out:

```js
const fit = () => muxy.popover.resize(
  document.documentElement.scrollWidth,
  document.documentElement.scrollHeight,
);
window.addEventListener('load', fit);
```

The popover also dismisses on outside click, and closes automatically when the extension is disabled or stopped. Opening a [dialog](dialogs.md) keeps the popover open so the dialog's result reaches the page.

## Theming

The popover renders over native macOS popover material with a transparent webview backing. Keep the page background transparent (`body { background: transparent; }`) so the system material — already light/dark aware — shows through. Use the injected `--muxy-*` theme variables for text, accents, and translucent `--muxy-surface` chips, as in [tabs](tabs.md) and [panels](panels.md).
