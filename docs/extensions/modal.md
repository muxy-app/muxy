# Extension Modal

A native, searchable picker overlay. The extension supplies a list; Muxy owns the UI, the search field, keyboard navigation, and open/close. Selecting an item (click or Return) delivers that item; dismissing (Esc, click outside) delivers `null`.

`modal` is available on all three surfaces: webview pages (tabs, panels, popovers) via [`window.muxy`](tabs.md#windowmuxy), [`runScript`](scripts.md) palette-command scripts via `muxy`, and the [background script](manifest.md) `muxy` global. It needs **no permission** — the user drives every selection themselves, so there is nothing to gate ([what permissions don't gate](permissions.md#what-permissions-dont-gate)).

**Delivery of the choice is via an `onSelect(choice)` callback**, which fires when the user picks or dismisses. On `runScript` and background scripts `modal.open` returns immediately (it does **not** block); `onSelect` is the only way to read the result. On webview pages `modal.open` also returns a `Promise` of the choice, so you may `await` it instead of using `onSelect`.

## open

Opens the picker with your items; `onSelect` receives the **selected item**, or `null` if dismissed.

```js
muxy.modal.open({
  placeholder: 'Pick a fruit...',   // search field placeholder
  emptyLabel: 'No items',           // shown when the list is empty
  noMatchLabel: 'No matches',       // shown when the query matches nothing
  items: [
    { id: 'apple', title: 'Apple', subtitle: 'Crisp and red' },
    { id: 'banana', title: 'Banana' },
  ],
  onSelect(choice) {
    if (choice) { /* choice = { id, title, subtitle } */ }
  },
});
```

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `items` | object[] or function | yes | The rows to show — an array, or an `items(emit)` producer (see [Streaming](#streaming-large-lists-items-producer)). |
| `onSelect` | function | no* | `onSelect(choice)` fires with the chosen item or `null`. Required on `runScript`/background (which don't return the choice); optional on webview where you can `await` the result instead. |
| `onQuery` | function | no | `onQuery(query, emit)` fires when the search text changes, letting you supply a fresh list per query (async/server-side search — see [Dynamic results](#dynamic-results-onquery)). |
| `placeholder` | string | no | Search field placeholder. Defaults to `"Search..."`. |
| `emptyLabel` | string | no | Message when there are no items. Defaults to `"No items"`. |
| `noMatchLabel` | string | no | Message when the query matches nothing. Defaults to `"No matches"`. |

Each item:

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `id` | string | yes | Returned to you on selection; identify the choice by this. |
| `title` | string | yes | The bold primary line. |
| `subtitle` | string | no | The dimmed secondary line. |

Muxy filters the list as the user types (case-insensitive substring match on `title` and `subtitle`), highlights with the arrow keys, and selects on Return or click. **Filtering is native** — by default, once your items are supplied, typing never calls back into your code, so search stays instant and the UI can never hang no matter how large the list or how fast the user types. To drive results from the query yourself (async or server-side search), opt in with [`onQuery`](#dynamic-results-onquery); native filtering still runs on top of whatever you supply.

## Streaming large lists (`items` producer)

A static `items` array enumerates everything upfront — fine for small lists, but for a big repo you
don't want to block the open while you gather every file. Instead pass **`items` as a function**.
Muxy opens the picker immediately (with a spinner) and calls your producer **once**, off the UI
thread; you push rows in via `emit(batch)` and the list fills as they arrive. The user can type
against whatever has loaded so far, and Muxy filters it natively.

```js
muxy.modal.open({
  placeholder: 'Open file…',
  items(emit) {
    const files = listAllFiles();             // you own the enumeration
    for (const chunk of batches(files, 5000)) {
      emit(chunk.map(f => ({ id: f.path, title: f.name, subtitle: f.path })));
    }
  },
  onSelect(choice) {
    if (choice) { /* { id, title, subtitle } */ }
  },
});
```

| `items` form | Behavior |
| --- | --- |
| array | The full list, supplied at once. Best for small, bounded sets. |
| `items(emit)` function | Called once. Call `emit(batchArray)` any number of times to stream rows; you may also just **return** the full array instead of emitting. The picker opens before this finishes. |

- `emit` takes an array of `{ id, title, subtitle? }` (entries missing `id`/`title` are dropped).
  Returning an array from the producer is equivalent to emitting it once.
- On webview pages the producer may be `async` (do `await emit(...)`); in `runScript` and background
  scripts it runs synchronously — call `muxy.exec`, `muxy.files.*`, etc. directly and emit. Filtering
  is always native, so `modal.open` never blocks on it; the choice arrives via `onSelect`.
- The dataset is capped at 100,000 rows; `id`, `title`, and `subtitle` are capped at 200 chars
  each. Producing nothing just shows the empty label.
- Because filtering is native, you never debounce or handle the query yourself — Muxy owns search,
  paging, and cancellation. For a static `items` list there is no per-keystroke callback; opt into
  [`onQuery`](#dynamic-results-onquery) when you want to feed results from the query.

## Dynamic results (`onQuery`)

A static `items` list (or a producer that runs once) is filtered natively and never calls back. When
the result set depends on the query itself — a server-side search, a remote API, a fuzzy index you own —
pass an **`onQuery(query, emit)`** handler. Muxy debounces the search field and calls `onQuery` with the
current text on every change; you return (or `emit`) the rows for that query, and Muxy swaps them in.
Native substring filtering still runs on top of whatever you supply, so partial matches within your
result set keep working.

```js
// background.js — shell out per query (background scripts have muxy.exec, not muxy.http)
muxy.modal.open({
  placeholder: 'Search npm…',
  items: [],                                  // initial list before the user types
  onQuery(query) {
    if (!query) return [];
    const out = muxy.exec(['curl', '-s', `https://registry.example/-/v1/search?text=${encodeURIComponent(query)}`]);
    const data = JSON.parse(out.stdout || '{}');
    return (data.objects || []).map(o => ({ id: o.package.name, title: o.package.name, subtitle: o.package.description }));
  },
  onSelect(choice) {
    if (choice) muxy.notifications.notify({ title: 'Picked', body: choice.title });
  },
});
```

On a webview page (tab/panel/popover) `onQuery` may be `async` and use [`muxy.http.fetch`](http.md):

```js
async onQuery(query) {
  if (!query) return [];
  const res = await muxy.http.fetch(`https://registry.example/-/v1/search?text=${encodeURIComponent(query)}`);
  return res.body ? JSON.parse(res.body).objects.map(o => ({ id: o.package.name, title: o.package.name })) : [];
}
```

- `onQuery(query, emit)` receives the trimmed query string and an `emit(batch)` you can call to stream
  rows (same shape as the producer's `emit`); returning an array is equivalent to emitting it once.
- Each call replaces the list for that query. Muxy tags every call with a revision and drops responses
  for superseded queries, so a slow request that resolves late never overwrites a newer one.
- The initial `items` (array or producer) still supplies the list shown before the user types; `onQuery`
  takes over once the query changes, including when it is cleared back to empty.
- On webview pages `onQuery` may be `async` (do `await muxy.http.fetch(...)`); in `runScript` and
  background scripts it runs synchronously — call `muxy.exec`, `muxy.files.*`, etc. directly and return.
  The spinner shows while you fetch.
- The same caps apply: 100,000 rows; `id`/`title`/`subtitle` 200 chars each.

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
muxy.events.subscribe('command.pick', () => {
  muxy.modal.open({
    placeholder: 'Pick a fruit...',
    items: [
      { id: 'apple', title: 'Apple', subtitle: 'Crisp and red' },
      { id: 'banana', title: 'Banana', subtitle: 'Soft and yellow' },
    ],
    onSelect(choice) {
      if (choice) muxy.notifications.notify({ title: 'Picked', body: choice.title });
    },
  });
});
```

## Notes

- On `runScript`/background, `modal.open` returns immediately and the choice arrives via `onSelect`; the script does not block waiting for the user. On webview pages `modal.open` also returns a `Promise` you can `await`.
- Only one modal is shown at a time. Opening a new one while another is showing closes the existing modal — its `onSelect` fires with `null` — and presents the new picker.
- `placeholder` and the labels are capped at 200 characters; `id`, `title`, and `subtitle` per item at 200. The dataset (array or streamed via the producer) is capped at 100,000 rows; items missing `id` or `title` are dropped.
- The modal presents on the main Muxy window.
