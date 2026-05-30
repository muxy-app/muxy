# Extension Popovers

Popovers are transient webviews anchored to a [topbar](topbar.md) or [status bar](statusbar.md) item. Clicking the item opens the popover; clicking outside dismisses it. Unlike [panels](panels.md), a popover does not dock and is not persisted — it is the right surface for a quick, read-mostly view (usage meters, a status summary, a small action list).

At most **one extension popover is open at a time**. Opening another anchor's popover closes the current one.

## Declaring a popover

```json
{
  "name": "ai-usage",
  "version": "0.1.0",
  "permissions": ["panels:write"],
  "popovers": [
    {
      "id": "usage",
      "title": "AI Usage",
      "entry": "popovers/usage.html",
      "width": 320,
      "height": 360
    }
  ],
  "commands": [
    {
      "id": "open-usage",
      "title": "Open AI Usage",
      "action": { "kind": "openPopover", "popover": "usage" }
    }
  ],
  "statusBarItems": [
    { "id": "usage", "icon": "sparkles", "side": "right", "command": "open-usage" }
  ]
}
```

A popover is always reached through a topbar/status bar item whose `command` resolves to an `openPopover` action. The popover anchors to that exact item.

### Fields

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `id` | string | yes | Stable per extension. Referenced from an `openPopover` command. |
| `entry` | string | yes | Path relative to the extension directory. Must resolve inside the directory (no `..` traversal). |
| `title` | string | no | Available to the page; the popover itself is frameless (no host chrome). |
| `width` | number | no | Initial width in points. Defaults to `320`. |
| `height` | number | no | Initial height in points. Defaults to `360`. |
| `defaultData` | object | no | JSON payload exposed to the page as `window.muxy.data`. |

The loader validates that `entry` exists inside the extension directory, that popover ids are unique, and that `openPopover` commands reference a declared popover id.

## Sizing

The popover opens at its declared `width`/`height` and the page resizes it to fit its content via the `panels:write` API:

```ts
window.muxy.popover.resize(width, height): Promise<void>;
```

A common pattern is to report the document size once the content has laid out:

```js
const fit = () => muxy.popover.resize(
  document.documentElement.scrollWidth,
  document.documentElement.scrollHeight
);
window.addEventListener('load', fit);
```

The host clamps the reported size to a sane range.

## Theming

The popover is presented over the native macOS popover material, and the webview's backing is transparent. Leave the page background transparent (`body { background: transparent; }`) so the system material — already light/dark aware — shows through and the popover matches macOS. Use the injected `--muxy-*` theme variables for foreground text, accents, and translucent `--muxy-surface` chips/buttons, exactly as in [tabs](tabs.md) and [panels](panels.md).

## Closing

The popover dismisses on outside click, or when the page asks the host to close it:

```ts
window.muxy.popover.close(): Promise<void>;
```

From an entrypoint subprocess over the socket:

```
popover.resize|<width>|<height>
popover.close
```

`popover.resize` and `popover.close` act on the popover currently open for the calling extension; there is no `open` verb because popovers are user-triggered from their anchor. Both require the `panels:write` permission. Popovers close automatically when the extension is disabled or stopped.
