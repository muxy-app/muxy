# Inline Scripts (`runScript` Commands)

A palette command with `action.kind = "runScript"` runs a JavaScript file in an in-process JavaScriptCore context when the user picks it. The script has the same `muxy.*` API as webview tabs, minus DOM, theme, data, and events. Requires the `commands:run-script` permission.

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

## Lifecycle

- The `JSContext` is created on first run and **cached for the extension's lifetime**, so `var`/`function` defined in one run remain visible to the next.
- It is **evicted** when the extension is disabled or reloaded (Settings → Extensions → Reload Extensions).
- The script **source is re-read from disk on every run**, so edits apply on the next palette trigger with no restart.

## API surface

`muxy.extensionID` plus the same synchronous methods as webview tabs:

```
muxy.notifications.notify(opts)      // alias: muxy.toast(opts)
muxy.tabs.{list, switchTo, new, next, previous, open}
muxy.panes.{list, send, sendKeys, readScreen, close, rename}
muxy.projects.{list, switchTo}
muxy.worktrees.{list, switchTo, refresh}
```

Plus `muxy.exec(argv, options?)` / `muxy.exec({ shell, ... })` to run shell commands (requires `commands:exec`):

```js
const status = muxy.exec(['git', 'status', '--short']);
console.log(status.stdout);
```

Differences from the webview API:

- All calls are **synchronous** — they return values directly, not Promises. Muxy blocks the script's own dispatch queue while the work runs on the main actor, so the UI stays responsive.
- No `muxy.theme`, `muxy.data`, or `muxy.tabInstanceID` — scripts have no tab or rendering surface.
- No `muxy.events.subscribe` or `muxy.events.emit` — scripts are one-shot.

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
