# Storage

`muxy.storage` is a small per-extension key/value store for persisting an extension's own state — layout, collapse state, preferences — without shelling out to a file. Each extension gets an isolated namespace; one extension can never read or write another's keys.

Values are persisted by Muxy to a private JSON file keyed by extension id (under Application Support), so they survive app restarts. Keys are strings; values are any JSON-serializable value (object, array, string, number, boolean, or `null`).

`storage` is available on webview pages (tabs, panels, popovers) via [`window.muxy`](tabs.md#windowmuxy), in [`runScript`](scripts.md) commands, and in the [background script](manifest.md). On webview pages the methods return a `Promise` (use `await`); in `runScript` and background scripts they are synchronous.

## Permissions

| Permission | Methods |
| --- | --- |
| `storage:read` | `get`, `keys` |
| `storage:write` | `set`, `delete` |

```json
{
  "permissions": ["storage:read", "storage:write"]
}
```

## Methods

```js
await muxy.storage.set('layout', { collapsed: ['groupA'], order: ['p1', 'p2'] });
const layout = await muxy.storage.get('layout'); // the stored value, or null if absent
const keys = await muxy.storage.keys();          // ['layout']
await muxy.storage.delete('layout');
```

| Method | Returns | Notes |
| --- | --- | --- |
| `get(key)` | the stored value, or `null` if the key is absent | |
| `set(key, value)` | — | `value` may be any JSON-serializable value. |
| `delete(key)` | — | No-op if the key is absent. |
| `keys()` | array of stored keys, sorted | |

## Limits

- A key is capped at 256 characters and must be non-empty.
- A single value is capped at 1 MB; the whole per-extension store is capped at 5 MB. Exceeding either rejects the `set`.
- The store is **per extension**, not per surface — a panel and the background script of the same extension share it.
