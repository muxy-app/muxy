# Inline Scripts (`runScript` Commands)

A palette command with `action.kind = "runScript"` runs a JavaScript file in a per-extension JavaScriptCore context. The script has access to the same `muxy.*` API as webview tabs — minus DOM and theme — and is executed when the user picks the command from the palette.

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
muxy.toast({
  title: 'Pane audit',
  body: `${panes.length} pane(s) — focused: ${panes.find(p => p.isFocused)?.title ?? 'none'}`,
});
```

## Lifecycle

- The first time a script for an extension runs, Muxy creates a `JSContext` and a dedicated dispatch queue for it.
- The context is **cached** for the extension's lifetime. Subsequent invocations reuse it, so any `var` / `function` defined in a previous run is still visible.
- The context is **evicted** when the extension is disabled or reloaded (Settings → Extensions → Reload Extensions).
- The script source is read fresh from disk on every invocation, so editing the file picks up on the next palette trigger — no app restart.

## API surface

`muxy.extensionID` plus the same methods as webview tabs:

```
muxy.toast(opts)
muxy.tabs.{list, switchTo, new, next, previous, open}
muxy.panes.{list, send, sendKeys, readScreen, close, rename}
muxy.projects.{list, switchTo}
muxy.worktrees.{list, switchTo, refresh}
```

Plus `muxy.exec(argv, options?)` / `muxy.exec({ shell, ... })` for running shell commands (requires `commands:exec`):

```js
const status = muxy.exec(['git', 'status', '--short']);
console.log(status.stdout);
```

**Differences from the webview API:**

- All calls are **synchronous** — `muxy.panes.list()` and `muxy.exec(...)` return values directly, not Promises. Internally Muxy blocks the script's dispatch queue while the async work runs on the main actor; the main thread is not blocked, so the UI stays responsive.
- No `muxy.theme`, `muxy.onThemeChange`, `muxy.data`, or `muxy.tabInstanceID` — scripts are not tied to a tab and have no rendering surface.
- No `muxy.events.subscribe` — scripts are strictly one-shot.

## Permissions

Running a `runScript` command requires `commands:run-script`. Each verb the script calls (`muxy.panes.send`, etc.) is gated by its own permission as on every other surface. If the script calls a method without the matching permission, the method throws an `Error("permission denied (<perm>)")` that the script can catch.

## Errors and logging

- `console.log`, `console.warn`, and `console.error` are bridged to the extension's [log file](logs.md), tagged `[log]`, `[warn]`, `[err]`.
- If the script throws, the error message is appended as `[err]` and the failed run is recorded with a `[muxy] runScript failed` line.
- If the script file is missing, the run is skipped and logged.

## When to use a script vs. a webview tab

| Use `runScript` when | Use a webview tab when |
| --- | --- |
| You're acting on workspace state and don't need UI | You need to render anything |
| The work fits in one shot — fire and forget | You want long-lived per-instance state |
| You want shared module-like state across invocations of *one* extension | You need DOM events, forms, charts, etc. |
