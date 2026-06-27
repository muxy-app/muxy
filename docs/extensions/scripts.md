# Inline Scripts (`runScript` Commands)

A palette command with `action.kind = "runScript"` runs a JavaScript file in an in-process JavaScriptCore context when the user picks it. The script gets a **synchronous** `muxy.*` API: it can read and act on workspace state (tabs, panes, projects, worktrees, agents, files, git), run shell commands, and present native UI (dialogs, modals, toasts, notifications, topbar/status-bar items) — all without a rendering surface. Requires the `commands:run-script` permission.

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

A script can also present native UI and act on the choice via `onSelect` — no background listener,
tab, or panel needed. `modal.open` returns immediately; the choice arrives in the callback:

```js
muxy.modal.open({
  placeholder: 'Switch to worktree…',
  items: muxy.worktrees.list().map(w => ({ id: w.id, title: w.name, subtitle: w.branch })),
  onSelect(choice) { if (choice) muxy.worktrees.switchTo(choice.id); },
});
```

For large lists (e.g. a file picker over a big repo), pass `items` as a producer function instead
of an array so the picker opens instantly and you stream rows in while Muxy filters them natively —
see [Modal → Streaming large lists](modal.md#streaming-large-lists-items-producer):

```js
muxy.modal.open({
  placeholder: 'Open file…',
  items(emit) {
    const out = muxy.exec(['git', 'ls-files']).stdout.split('\n');
    let batch = [];
    for (const line of out) {
      const p = line.trim();
      if (!p) continue;
      batch.push({ id: p, title: p.split('/').pop(), subtitle: p });
      if (batch.length >= 5000) { emit(batch); batch = []; }
    }
    if (batch.length) emit(batch);
  },
  onSelect(choice) {
    if (!choice) return;
    muxy.tabs.open({
      kind: 'extensionWebView',
      extension: {
        id: muxy.extensionID,
        tabType: 'editor',
        singleton: true,
        data: { path: choice.id },
      },
    });
  },
});
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
muxy.projects.{list, switchTo, add, rename, setColor, setIcon, setLogo, reorder, delete}
muxy.worktrees.{list, switchTo, refresh}
muxy.browser.{open, navigate, list, read, close, eval, click, type, waitFor, …}  // requires browser:read / browser:write
muxy.agents.list()                                              // requires agents:read
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
- No rendering/tab surface: no `muxy.data`, `muxy.theme`, `muxy.onDataChange`, `muxy.onThemeChange`, `muxy.focused`, `muxy.onFocus`, or `muxy.tabInstanceID`.
- No page-only APIs: no `muxy.panels`, `muxy.popover`, `muxy.http`, or `muxy.tabs.setTitle`/`setIcon` (those need a tab instance).
- No `muxy.events` and no `muxy.remote` — those are background-script APIs ([events](events.md), [remote methods](remote-methods.md)).

## Remote workspaces

When the active workspace is a remote (SSH) workspace, `muxy.exec`, `muxy.git.*`, and worktree operations execute **on the remote server**, not the Mac. Paths (`cwd`, project/worktree paths) are remote paths. Muxy brokers the SSH connection for you using your system SSH config, keys, and agent — the extension code is unchanged whether the active workspace is local or remote.

Remote commands inherit the selected SSH device's environment. New SSH devices default to `TERM=xterm-256color`; users can edit these variables in Settings -> Remote Devices.

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
