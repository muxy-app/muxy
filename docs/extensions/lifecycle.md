# Lifecycle

[Events](events.md) tell you a surface *did* something. Lifecycle lets your surface **act before it closes** — most importantly, prevent the close. The canonical case: a file editor that refuses to close while the file is dirty and shows its own Save / Don't Save / Cancel dialog instead.

`muxy.lifecycle` is available on tab, panel, popover, and [webview modal](modal.md#webview-modal-openwebview) pages. There is no manifest field and no permission to declare: registering a handler is the opt-in.

## Intercepting a close

Register a handler from the surface's own page. Before Muxy closes that surface, it calls your handler and waits for the verdict.

```js
muxy.lifecycle.onBeforeClose(async () => {
  if (!isDirty) return false;            // allow the close

  const choice = await muxy.dialog.confirm({
    title: 'Unsaved changes',
    message: 'Save before closing?',
    buttons: ['Save', "Don't Save", 'Cancel'],
  });

  if (choice === 'Cancel') return true;  // PREVENT the close
  if (choice === 'Save') await save();
  return false;                           // allow — the close proceeds
});
```

- Return (or resolve) **`true`** — or **`{ prevent: true }`** — to **prevent** the close.
- Return anything else (`false`, `undefined`, …) to **allow** it.
- The handler may be sync, return a Promise, or be `async` (so you can `await` your own dialog or a save).
- `onBeforeClose` returns an unsubscribe function. Registering again replaces the handler — there is one per surface.

The handler receives a small context: `{ surface: 'tab' | 'panel' | 'popover' | 'modalWebview', instanceID }`.

## Closing yourself

When your handler has decided the close should happen (the user picked "Save" or "Don't Save"), finish it with:

```js
muxy.lifecycle.close();
```

This closes **this** surface and **bypasses** the veto — it will not ask `onBeforeClose` again, so there's no loop. Use it instead of returning `false` when you want to drive the close yourself after your own UI.

## Guarantees

- **Fail-open.** If you register no handler, your handler throws, or the page never responds, the close proceeds. A surface can never wedge the close button. A page that *has* a handler is given a few seconds to acknowledge the request; once it does, it may take as long as it needs (e.g. while a human reads a Save / Don't Save dialog) — the close waits for the verdict.
- **Scoped to your own surface.** A veto only ever delays or cancels a user-initiated close of the surface that registered the handler. It can't affect other tabs, panels, or extensions.
- **Bulk closes ask in parallel.** "Close Other Tabs" (and similar) ask every affected surface at once — you get one round of prompts, not a queue of blocking dialogs.

## Limits

- **Quitting the app and closing a window skip the veto** (the app's own quit confirmation governs there) — they only emit the observation events. Don't rely on `onBeforeClose` to guard against quit; persist on a timer or on `tab.focused`/blur instead.
- **Toggling a panel or popover closed is a show/hide, not a close** — it bypasses the veto. `onBeforeClose` fires for genuine close intents: the surface's close button, the programmatic `muxy.panels.close()` / `muxy.popover.close()` / `muxy.panes.close()`, and closing a tab. A topbar/command toggle that hides the surface does not ask, and `muxy.lifecycle.close()` deliberately bypasses it.
- **A popover dismissed by clicking outside it cannot be vetoed** — macOS has already torn it down. You still get `popover.closed`.

## Observing closes

To merely *react* after a close (not prevent it), subscribe to the observation events — `tab.closed`, `panel.closed`, `popover.closed`, and the open counterparts — see [Events](events.md). Those fire after the surface is gone; a prevented close emits nothing.
