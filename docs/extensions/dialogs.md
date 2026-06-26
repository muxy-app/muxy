# Extension Dialogs

Native macOS sheets an extension can present on the main window: a multi-button **confirm** dialog, a single-button **alert**, a single-field **prompt**, and a folder picker (**pickFolder**). Each renders as a real `NSAlert`/`NSOpenPanel` sheet attached to the Muxy window — they look identical to the app's own prompts and block until the user responds.

`dialog` is available on all three surfaces: webview pages (tabs, panels, popovers) via [`window.muxy`](tabs.md#windowmuxy), [`runScript`](scripts.md) palette-command scripts via `muxy`, and the [background script](manifest.md) `muxy` global. It needs **no permission** — the user has to dismiss every dialog themselves, so there is nothing to gate ([what permissions don't gate](permissions.md#what-permissions-dont-gate)).

On webview pages `confirm`/`alert` return a `Promise` — use `await`. In `runScript` and background scripts they are **synchronous** and return the result directly; `await` is harmless but not required. In every case the call blocks until the user responds.

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

## prompt

Shows a dialog with a single text field and resolves with the entered **string**, or `null` if the user cancelled (Esc or the cancel button).

```js
const name = await muxy.dialog.prompt({
  title: 'Rename worktree',
  message: 'Enter a new branch name.',
  default: 'feature/login',   // pre-filled text
  placeholder: 'branch name', // shown when empty
  confirm: 'Rename',          // confirm button label (default 'OK')
  cancel: 'Cancel',           // cancel button label (default 'Cancel')
});

if (name !== null) { /* … */ }
```

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `title` | string | one of title/message | The bold heading line. |
| `message` | string | one of title/message | The informative body text. |
| `default` | string | no | Pre-filled field value. |
| `placeholder` | string | no | Placeholder shown when the field is empty. |
| `confirm` | string | no | Confirm (Return) button label. Defaults to `'OK'`. |
| `cancel` | string | no | Cancel (Esc) button label. Defaults to `'Cancel'`. |

## pickFolder

Opens a native folder picker and resolves with the absolute **path** of the chosen directory, or `null` if cancelled.

```js
const folder = await muxy.dialog.pickFolder({
  title: 'Choose a project folder',
  message: 'Add',           // the panel's confirm button label
  default: '~/Projects',    // initial directory (tilde expanded)
});

if (folder !== null) { /* … */ }
```

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `title` | string | no | Message shown above the panel. |
| `message` | string | no | Confirm button label of the panel. |
| `default` | string | no | Initial directory; a leading `~` is expanded. |

## Notes

- The call blocks the caller until the user responds. From a background script this pauses that script's event loop the same way `exec` does, so don't open a dialog from a hot event path.
- Only **one dialog per extension** can be open at a time; a second call while one is showing rejects rather than stacking sheets.
- `title`, `message`, and each button label are capped at 2000 characters, and `buttons` is limited to the first 3.
- Dialogs present as a sheet on the main Muxy window; if no window is available the call rejects rather than presenting a blocking dialog.
- A [popover](popovers.md) stays open while its dialog is showing, so the resolved result reaches the popover page.
