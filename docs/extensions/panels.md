# Panels

Panels, popovers, and the sidebar are web pages around the workspace. They all
get the same page API, [data](tabs.md#page-data), and [theme](tabs.md#theme)
as tabs.

## Panels

A panel docks to the right or bottom of the workspace, slides over that edge,
or floats freely over the window as a [detached panel](#detached-panels).

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
| `mode` | `floating` (default, over the edge), `pinned` (beside the workspace), or `detached` (anywhere over the window). Users can switch, and their choice is kept. |
| `hiddenControls` | Hide header controls: `icon`, `title`, `close`, `pin`, `position`, `detach`. |
| `headerButtons` | Extra header buttons, shaped like [bar items](bar-items.md). |
| `hideTopbar` | Hide the whole header. |
| `defaultData` | Data used when opened without any. |

- Open one with a `togglePanel` or `openPanel` [command](commands.md), or from
  a page, script, or background script with `muxy.panels.open(id, data,
  options)`, `toggle(id, data, options)`, or `close(id, instance)`. `close()`
  with no ID closes the calling panel. Needs `panels:write`.
- Only one panel docks at a time: opening another docked or edge panel
  replaces it. Detached panels don't take that place.
- Each project remembers its open docked panel: switching projects closes it,
  and switching back reopens it.
- A panel over the edge (`floating`) closes when you click elsewhere.
- **Pop Out Panel** in the header detaches a docked panel, and **Dock Panel**
  docks a detached one. Hide that control with `hiddenControls: ["detach"]`.

## Detached panels

A detached panel floats anywhere over the window, beside the docked panel, the
agents panel, and other detached panels. Clicking one brings it to the front.
It stays open until it is closed, and can be framed like a small window or have
no frame at all, so the page draws everything: a sticky note, a clock, or a pet
walking across the screen.

```json
"panels": [
  {
    "id": "note",
    "title": "Note",
    "entry": "note/index.html",
    "mode": "detached",
    "frame": { "x": 120, "y": 120, "width": 240, "height": 200 },
    "minSize": { "width": 160, "height": 120 }
  },
  {
    "id": "pet",
    "entry": "pet/index.html",
    "mode": "detached",
    "chrome": "none",
    "clickThrough": true,
    "resizable": false,
    "frame": { "x": 40, "y": 500, "width": 96, "height": 96 }
  }
]
```

| Field | Notes |
| --- | --- |
| `frame` | `{ x, y, width, height }` where it first opens, from the window's top left corner, in the page's CSS pixels. Any part can be left out. |
| `minSize`, `maxSize` | `{ width, height }` limits for resizing. The smallest default is 24 × 24. |
| `chrome` | `standard` (default) draws a card with a header that moves it. `none` draws nothing: the page fills the frame. |
| `transparent` | Nothing is drawn behind the page, so what is under it shows where the page is transparent, such as around rounded corners. On by default with `chrome: "none"`, off otherwise. Give the page `background: transparent`. Fixed once the panel opens. |
| `clickThrough` | Clicks pass through the panel to what is under it, except over its [hit regions](#clicks-and-dragging). |
| `movable`, `resizable` | Whether the user can move it by its header or drag regions, and resize it by its edges and corners. Both default to `true`. |
| `scope` | `global` (default) shows it in every project. `project` shows it only in one project, and keeps it alive, hidden, while you are elsewhere. That is the project it was opened in; opening it again, or setting `scope: "project"`, moves it to the current one. |

Every field above is a default. An extension can change all of them while the
panel is open, except `transparent`.

- The frame is fitted inside the window, and moved back in when the window
  shrinks. With `chrome: "standard"`, the header takes the top of the frame.
- Muxy remembers where the user leaves a panel opened without an instance name.
  Instances are the extension's to remember, with `onChange` below.
- An extension keeps at most 32 panels open, docked and detached together.

### Instances

Open one panel several times by naming each copy. Each instance has its own
page, data, frame, and options.

```js
await muxy.panels.open("note", { text: "" }, { instance: "note-3", frame: { x: 300, y: 80 } });
await muxy.panels.update("note", { title: "Groceries" }, "note-3");
await muxy.panels.close("note", "note-3");
const open = await muxy.panels.list();
```

An instance name is 1 to 64 letters, digits, `.`, `_`, or `-`. A page reads its
own as `muxy.panel.instanceID`, and `lifecycle.onBeforeClose` handlers get it as
`panelInstanceID`. `panel.opened` and `panel.closed` [events](events.md) carry it
as `instance`. `list()` returns the extension's open panels: docked ones, and
detached ones even while another project hides them. A named copy that another
docked panel replaces is closed.

### Options

`open`, `toggle`, `update`, and `muxy.panel.update` take the same options.
Fields left out keep their current value.

| Option | Notes |
| --- | --- |
| `instance` | Which copy to open (in `open` and `toggle`). |
| `mode` | `pinned`, `floating`, or `detached`. Moves an open panel between docked and detached. |
| `position` | `right` or `bottom`, where it docks. |
| `frame` | `{ x, y, width, height }`, any part. |
| `minSize`, `maxSize` | `{ width, height }` |
| `chrome`, `clickThrough`, `movable`, `resizable`, `scope` | As in the manifest. |
| `transparent` | Only when the instance opens. |
| `title` | The header title; `null` clears it. |
| `hiddenControls` | Replaces the manifest's list. |
| `hitRegions` | Rectangles where the page takes clicks. `null` removes them: the page then takes clicks everywhere, or nowhere while `clickThrough` is on. |
| `dragRegions` | Rectangles that move the panel when dragged. |

### From the panel itself

A panel page controls its own panel with `muxy.panel`. It is `null` on other
pages.

```js
muxy.panel.setFrame({ width: 320, height: 140 });       // grow to show a speech bubble
muxy.panel.setOptions({ chrome: "none", movable: true });
muxy.panel.setDragRegions([{ x: 0, y: 0, width: 96, height: 96 }]);
muxy.panel.raise();
muxy.panel.onChange(state => save(state.frame));
```

| Member | What it does |
| --- | --- |
| `instanceID` | This copy's instance name, or `null` |
| `update(options)`, `setOptions(options)` | Changes any [option](#options) |
| `setFrame(frame)`, `setTitle(title)` | Shortcuts for `update` |
| `setHitRegions(rects)`, `setDragRegions(rects)` | See below |
| `trackHitElements(target, { padding, interval })` | Keeps the hit regions on elements as they move and resize. `target` is a CSS selector, an element, a list, or a function returning elements. Returns a function that stops tracking. |
| `raise()` | Brings the panel to the front |
| `get()`, `state` | The panel's state, below |
| `onChange(fn)` | Called with the new state whenever it changes, such as after the user moves it or the window resizes |

The state is `{ panelID, instance, mode, position, visible, frame, bounds,
scope, chrome, transparent, clickThrough, movable, resizable, title, focused }`.
`bounds` is the window's `{ width, height }`, the space a detached panel can
use.

### Clicks and dragging

Rectangles are in the page's CSS pixels, from its top left corner.

- **Hit regions.** With `clickThrough`, or once hit regions are set, the page
  takes clicks only inside its hit regions. Everywhere else, clicks and scrolling
  reach the terminal, tab, or panel underneath. A pet sets its hit region to
  its body, and to its speech bubble while one shows.
- **Drag regions.** Dragging inside one moves the panel. A frameless panel uses
  them as its handle. The page doesn't get those clicks.

```js
// A pet that wanders, and can be clicked and dragged.
const pet = document.querySelector("#pet");
muxy.panel.trackHitElements("#pet, .bubble");
muxy.panel.setDragRegions([{ x: 0, y: 0, width: 96, height: 96 }]);
pet.addEventListener("click", async () => {
  const { frame } = await muxy.panel.get();
  await muxy.panel.setFrame({ width: 280, height: frame.height + 80, y: frame.y - 80 });
  showBubble("Hi!");
});
```

## Popovers

A popover opens under a top bar item, left of a rail item, or above a status
bar item, and closes when you click outside it.

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

Users turn it on in **Settings → Appearance → Active sidebar**, which lists the
built-in sidebar and every enabled extension that declares one. If the chosen
extension is disabled or removed, the built-in sidebar shows until it returns.
It can't close itself. The manifest and the choice carry over from Muxy 1.x
unchanged.
