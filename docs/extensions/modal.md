# Extension Modal

A native, searchable picker overlay. The extension supplies a list; Muxy owns the UI, the search field, keyboard navigation, and open/close. Selecting an item (click or Return) closes the modal and resolves the call with that item; dismissing (Esc, click outside) resolves with `null`.

`modal` is available on all three surfaces: webview pages (tabs, panels, popovers) via [`window.muxy`](tabs.md#windowmuxy), [`runScript`](scripts.md) palette-command scripts via `muxy`, and the [background script](manifest.md) `muxy` global. It needs **no permission** — the user drives every selection themselves, so there is nothing to gate ([what permissions don't gate](permissions.md#what-permissions-dont-gate)).

On webview pages `modal.open` returns a `Promise` — use `await`. In `runScript` and background scripts it is **synchronous** and returns the selected item (or `null`) directly; `await` is harmless but not required. In every case the call blocks until the user responds.

## open

Opens the picker with your items and resolves with the **selected item**, or `null` if dismissed.

```js
const choice = await muxy.modal.open({
  placeholder: 'Pick a fruit...',   // search field placeholder
  emptyLabel: 'No items',           // shown when the list is empty
  noMatchLabel: 'No matches',       // shown when the query matches nothing
  items: [
    { id: 'apple', title: 'Apple', subtitle: 'Crisp and red' },
    { id: 'banana', title: 'Banana' },
  ],
});

if (choice) { /* choice = { id, title, subtitle } */ }
```

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `items` | object[] | yes | The rows to show. Each needs an `id` and `title`; `subtitle` is optional. |
| `placeholder` | string | no | Search field placeholder. Defaults to `"Search..."`. |
| `emptyLabel` | string | no | Message when there are no items. Defaults to `"No items"`. |
| `noMatchLabel` | string | no | Message when the query matches nothing. Defaults to `"No matches"`. |

Each item:

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `id` | string | yes | Returned to you on selection; identify the choice by this. |
| `title` | string | yes | The bold primary line. |
| `subtitle` | string | no | The dimmed secondary line. |

Muxy filters the list as the user types (case-insensitive substring match on `title` and `subtitle`), highlights with the arrow keys, and selects on Return or click.

## Lazy provider (`search`)

Passing `items` enumerates everything upfront — fine for small, bounded lists, but it stalls on
large sets (e.g. every file in a big repo). Instead pass a **`search` function** and Muxy pulls
results on demand: it calls `search` on each keystroke (debounced) with the query and a paging
window, shows a spinner while it awaits, streams the rows in, and asks for the next page as the
user scrolls. You only ever compute the page Muxy asks for — nothing is loaded until it's needed.

```js
const choice = await muxy.modal.open({
  placeholder: 'Open file…',
  async search(query, { offset, limit }) {
    const files = await findFiles(query, offset, limit);   // you own the search
    return {
      items: files.map(f => ({ id: f.path, title: f.name, subtitle: f.path })),
      hasMore: files.length === limit,   // tell Muxy a next page may exist
    };
  },
});
if (choice) { /* { id, title, subtitle } */ }
```

| `search` arg | Type | Notes |
| --- | --- | --- |
| `query` | string | The current search text (empty on first open). |
| `offset` | number | Index of the first row Muxy wants (0 on a fresh query, grows as you scroll). |
| `limit` | number | How many rows to return for this page. |

`search` returns `{ items, hasMore }` (or a bare `items` array). `items` is the same
`{ id, title, subtitle? }` shape; `hasMore: true` lets Muxy request the next page when the user
scrolls to the bottom. Provide **either** `items` **or** `search`, not both — if `search` is
present it wins.

- On webview pages and the background script, `search` may be `async` (return a `Promise`).
- In `runScript` the whole surface is synchronous, so `search` runs **synchronously** — call
  `muxy.exec`, `muxy.files.*`, etc. directly and return the page. `modal.open` still blocks and
  returns the selection inline, exactly like the eager form.
- Each page is capped at 1000 items / 200 chars per field; the **total** set is unbounded because
  it is never materialized at once. A page that throws (or returns nothing) just shows no rows for
  that query.

## Opening from a shortcut

The modal has no shortcut of its own — wire one through a [palette command](palette-commands.md). Declare a command with a `defaultShortcut`, listen for its event in `background.js`, then open the modal:

```json
{
  "muxy": {
    "background": "background.js",
    "permissions": ["notifications:write"],
    "events": ["command.pick"],
    "commands": [
      { "id": "pick", "title": "Pick an Item", "action": { "kind": "event" }, "defaultShortcut": "cmd+shift+m" }
    ]
  }
}
```

```js
// background.js
muxy.events.subscribe('command.pick', async () => {
  const choice = await muxy.modal.open({
    placeholder: 'Pick a fruit...',
    items: [
      { id: 'apple', title: 'Apple', subtitle: 'Crisp and red' },
      { id: 'banana', title: 'Banana', subtitle: 'Soft and yellow' },
    ],
  });
  if (choice) muxy.notifications.notify({ title: 'Picked', body: choice.title });
});
```

## Notes

- The call blocks the caller until the user responds. From a background script this pauses that script's event loop the same way `exec` does, so don't open a modal from a hot event path.
- Only one modal is shown at a time. Opening a new one while another is showing closes the existing modal — its pending call resolves with `null` — and presents the new picker.
- `placeholder` and the labels are capped at 200 characters; `id`, `title`, and `subtitle` per item at 200. The eager `items` list is capped at the first 1000; a `search` page is capped at 1000 rows per call but the overall result set is unbounded. Items missing `id` or `title` are dropped.
- The modal presents on the main Muxy window; if no item survives validation the call rejects.
