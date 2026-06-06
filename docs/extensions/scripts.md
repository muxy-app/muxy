# Inline Scripts (`runScript` Commands)

A palette command with `action.kind = "runScript"` runs a JavaScript file in an in-process JavaScriptCore context when the user picks it. The script gets a **synchronous** `muxy.*` API: it can read and act on workspace state (tabs, panes, projects, worktrees, files, git), run shell commands, and present native UI (dialogs, modals, toasts, notifications, topbar/status-bar items) — all without a rendering surface. Requires the `commands:run-script` permission.

```json
{
  "permissions": ["commands:run-script", "panes:read", "notifications:write"],
  "commands": [
    {
      "id": "sync-panes",
      "title": "Sync: Audit panes",
      "action": { "kind": "runScript", "script": "scripts/sync.js" }
    }
  ]
}
```

```js
const panes = muxy.panes.list();
muxy.notifications.notify({
  title: 'Pane audit',
  body: `${panes.length} pane(s) — focused: ${panes.find(p => p.isFocused)?.title ?? 'none'}`,
});
```

A script can also present native UI and act on the choice inline — no background listener, tab, or panel needed:

```js
const choice = muxy.modal.open({
  placeholder: 'Switch to worktree…',
  items: muxy.worktrees.list().map(w => ({ id: w.id, title: w.name, subtitle: w.branch })),
});
if (choice) muxy.worktrees.switchTo(choice.id);
```

Note there is **no `await`** — see [API surface](#api-surface).

## Lifecycle

- The `JSContext` is created on first run and **cached for the extension's lifetime**, so `var`/`function` defined in one run remain visible to the next.
- It is **evicted** when the extension is disabled or reloaded (Settings → Extensions → Reload Extensions).
- The script **source is re-read from disk on every run**, so edits apply on the next palette trigger with no restart.

## API surface

`muxy.extensionID` plus the following methods. They are **synchronous** — they return values directly, no `await` (unlike the Promise-based webview bridge):

```
muxy.notifications.notify(opts)      // alias: muxy.toast(opts)
muxy.dialog.{confirm, alert}
muxy.modal.open(opts)
muxy.topbar.{set, show, hide}        // requires panels:write
muxy.statusbar.{set, show, hide}     // requires panels:write
muxy.tabs.{list, switchTo, new, next, previous, open}
muxy.panes.{list, send, sendKeys, readScreen, close, rename}
muxy.projects.{list, switchTo}
muxy.worktrees.{list, switchTo, refresh}
muxy.files.{list, read, stat, write, mkdir, rename, move, delete}
muxy.git.{status, diff, log, branches, commit, push, pull, …}   // full git surface, incl. git.pr.*, git.branch.*, git.worktree.*, git.tag.*
muxy.exec(argv, options?) / muxy.exec({ shell, ... })           // requires commands:exec
```

```js
const status = muxy.exec(['git', 'status', '--short']);
console.log(status.stdout);
```

Differences from the webview API:

- All calls are **synchronous** — they return values directly, not Promises. Muxy blocks the script's own dispatch queue while the work runs on the main actor, so the UI stays responsive.
- No rendering/tab surface: no `muxy.data`, `muxy.theme`, `muxy.onDataChange`, `muxy.onThemeChange`, or `muxy.tabInstanceID`.
- No page-only APIs: no `muxy.panels`, `muxy.popover`, `muxy.http`, or `muxy.tabs.setTitle`/`setIcon` (those need a tab instance).
- No `muxy.events` and no `muxy.remote` — those are background-script APIs ([events](events.md), [remote methods](remote-methods.md)).

## Permissions

Each verb is gated by its own permission, as on every surface (see [Permissions](permissions.md)). Calling a method without its permission throws `Error("permission denied (<perm>)")`, which the script can catch.

## Errors and logging

- `console.log`, `console.warn`, `console.error` are bridged to the extension's [log file](logs.md), tagged `[log]`, `[warn]`, `[err]`.
- A thrown error is logged as `[err]` plus a `[muxy] runScript failed` line. A missing script file is skipped and logged.

## When to use a script vs. a webview tab

| Use `runScript` when | Use a webview tab when |
| --- | --- |
| You act on workspace state and need no UI | You need to render anything |
| The work is fire-and-forget | You want long-lived per-instance state |
| You want module-like state shared across runs of *one* extension | You need DOM events, forms, charts, etc. |
