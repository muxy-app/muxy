# Extension Modal

A native, searchable picker — the same overlay as Muxy's own Quick Open (`Cmd+P`). The extension supplies a list; Muxy owns the UI, the search field, keyboard navigation, and open/close. Selecting an item (click or Return) closes the modal and resolves the call with that item; dismissing (Esc, click outside) resolves with `null`.

`modal` is available on both surfaces: the in-process [`window.muxy`](tabs.md#windowmuxy) bridge (tabs, panels, popovers) and the [background script](manifest.md) `muxy` global. It needs **no permission** — the user drives every selection themselves, so there is nothing to gate ([what permissions don't gate](permissions.md#what-permissions-dont-gate)).

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
- `placeholder` and the labels are capped at 200 characters; `id`, `title`, and `subtitle` per item at 200; the list at the first 1000 items. Items missing `id` or `title` are dropped.
- The modal presents on the main Muxy window; if no item survives validation the call rejects.
