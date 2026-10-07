# Bar Items

Extensions can add icons to the top bar and the status bar. Clicking one runs
one of the extension's [commands](commands.md).

```json
"topbarItems": [
  { "id": "hello", "icon": "sparkles", "command": "toggle", "tooltip": "Hello" }
],
"statusBarItems": [
  { "id": "count", "icon": "bolt", "text": "0", "side": "right", "command": "show-status" }
]
```

| Field | Notes |
| --- | --- |
| `id`, `icon`, `command` | Required. `icon` follows the [icon format](manifest.md#icons). |
| `tooltip` | Shown on hover. |
| `visible` | Default `true`. |
| `side` | Status bar only, required: `left` or `right`. |
| `text` | Status bar only: text next to the icon. |

Items appear grouped by extension, in manifest order. An item whose command
opens a popover anchors the popover to itself; see [Popovers](panels.md#popovers).

## Changing items at runtime

With `panels:write`, pages, scripts, and background scripts can change an item
they declared:

```js
muxy.statusbar.set({ id: "count", text: "3", icon: "bolt.fill" });
muxy.topbar.hide("hello");
muxy.topbar.show("hello");
```

`set` takes `id` plus any of `icon`, `visible`, and, for the status bar,
`text` (`null` clears it). Changes are not saved; items start from the
manifest again after a restart.
