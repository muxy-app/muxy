# Extension Dialogs

Native macOS sheets an extension can present on the main window: a multi-button **confirm** dialog and a single-button **alert**. Both render as a real `NSAlert` sheet attached to the Muxy window — they look identical to the app's own prompts and block until the user responds.

`dialog` is available on both surfaces: the in-process [`window.muxy`](tabs.md#windowmuxy) bridge (tabs, panels, popovers) and the [background script](manifest.md) `muxy` global. It needs **no permission** — the user has to dismiss every dialog themselves, so there is nothing to gate ([what permissions don't gate](permissions.md#what-permissions-dont-gate)).

## confirm

Shows a dialog with up to your supplied buttons and resolves with the **label** of the button the user clicked, or `null` if they cancelled (Esc, or the `cancel` button).

```js
const choice = await muxy.dialog.confirm({
  title: 'Delete branch?',
  message: 'This permanently removes feature/login and cannot be undone.',
  buttons: ['Delete', 'Cancel'],
  default: 'Cancel',   // the focused (Return) button
  cancel: 'Cancel',    // Esc maps here; clicking it resolves null
  style: 'warning',    // 'info' (default) | 'warning' | 'critical'
});

if (choice === 'Delete') { /* … */ }
```

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `title` | string | one of title/message | The bold heading line. |
| `message` | string | one of title/message | The informative body text. |
| `buttons` | string[] | no | Button labels, left to right. Defaults to `['OK', 'Cancel']`. |
| `default` | string | no | Label of the button focused and triggered by Return. Moved to the front so macOS highlights it. |
| `cancel` | string | no | Label that Esc maps to; clicking it resolves `null` instead of the label. |
| `style` | string | no | `'info'`, `'warning'`, or `'critical'`. Defaults to `'info'`. |

The resolved value is always the exact label string you passed (or `null`), so compare against your own labels rather than an index.

## alert

Shows a single **OK** dialog and resolves once the user dismisses it.

```js
await muxy.dialog.alert({
  title: 'Build finished',
  message: 'All 42 tests passed.',
  style: 'info',
});
```

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `title` | string | one of title/message | The bold heading line. |
| `message` | string | one of title/message | The informative body text. |
| `style` | string | no | `'info'`, `'warning'`, or `'critical'`. Defaults to `'info'`. |

## Notes

- The call blocks the caller until the user responds. From a background script this pauses that script's event loop the same way `exec` does, so don't open a dialog from a hot event path.
- Only **one dialog per extension** can be open at a time; a second call while one is showing rejects rather than stacking sheets.
- `title`, `message`, and each button label are capped at 2000 characters, and `buttons` is limited to the first 3.
- Dialogs present as a sheet on the main Muxy window; if no window is available the call rejects rather than presenting a blocking dialog.
