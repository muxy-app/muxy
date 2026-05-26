# Extension Tabs

Extensions can register custom tab types that render HTML/CSS/JS inside Muxy. Each opened tab is its own `WKWebView`. The Muxy host injects a `window.muxy` JavaScript API that calls the same typed `MuxyAPI` layer as the socket and the CLI — same permission gates apply.

## Declaring a tab type

```json
{
  "name": "pr-tools",
  "version": "0.1.0",
  "entrypoint": "run.sh",
  "permissions": ["tabs:write", "notifications:write"],
  "tabTypes": [
    {
      "id": "pr-viewer",
      "title": "PR Viewer",
      "entry": "tabs/pr.html",
      "defaultData": { "mode": "compact" }
    }
  ],
  "commands": [
    {
      "id": "open-pr",
      "title": "Open PR…",
      "action": { "kind": "openTab", "tabType": "pr-viewer" }
    }
  ]
}
```

### Fields

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `id` | string | yes | Stable per extension. Used to reference the tab type from commands and from `muxy.tabs.open()`. |
| `title` | string | yes | Default tab title. Used until the page sets a `customTitle`. |
| `entry` | string | yes | Path relative to the extension directory. Must resolve inside the directory (no `..` traversal). |
| `defaultData` | object | no | JSON payload merged into `window.muxy.data` when no explicit data is passed. |

The loader validates that `entry` exists and lives inside the extension directory; openTab commands must reference a declared tab type id.

## Asset loading

The webview loads its entry HTML at `muxy-ext://<extensionID>/<entry>` and the page can reference its own files with relative paths (`<link href="styles.css">`, `<script src="app.js">`). The `muxy-ext://` scheme is only registered inside the webview's configuration — it isn't a system-wide URL handler — and the scheme handler is locked to that one extension's directory.

## window.muxy

The native side injects `window.muxy` at document start, before any of the page's scripts run. Every method returns a `Promise`.

```ts
window.muxy = {
  extensionID: string,
  tabInstanceID: string,
  data: object | null,         // the payload the tab was opened with

  toast({ title, body?, paneID? }): Promise<void>,

  tabs: {
    open(request): Promise<void>,      // { kind, filePath?, extension? }
    list(): Promise<TabInfo[]>,
    switchTo(idOrIndex): Promise<void>,
    new(): Promise<string | null>,
    next(): Promise<void>,
    previous(): Promise<void>,
  },

  panes: {
    list(): Promise<PaneInfo[]>,
    send(paneID, text): Promise<void>,
    sendKeys(paneID, key): Promise<void>,
    readScreen(paneID, lines?): Promise<string>,
    close(paneID): Promise<void>,
    rename(paneID, title): Promise<void>,
  },

  projects: { list(), switchTo(identifier) },
  worktrees: { list(project?), switchTo(identifier, project?), refresh(project?) },
}
```

Each call requires the matching manifest permission — e.g. `panes.send` requires `panes:write`. Unauthorized calls reject with `Error("permission denied (panes:write)")`.

### Opening another tab

```js
// Open the editor on a specific file
await muxy.tabs.open({ kind: 'editor', filePath: '/path/to/foo.swift' });

// Open the VCS panel
await muxy.tabs.open({ kind: 'vcs' });

// Open an extension tab type (own or another extension's)
await muxy.tabs.open({
  kind: 'extensionWebView',
  extension: { id: 'pr-tools', tabType: 'pr-viewer', data: { prNumber: 42 } },
});
```

`extensionWebView` requires the target extension to be loaded and the named tab type to exist.

## Calling from the extension subprocess

The subprocess can open tabs over the socket too:

```
open-tab|{"kind":"extensionWebView","extension":{"id":"pr-tools","tabType":"pr-viewer","data":{"prNumber":42}}}
```

This requires `tabs:write` like any other tabs-mutating verb.

## Persistence

Workspace restoration persists the tab's `extensionID`, `tabTypeID`, and `data`. On restart, the tab reopens with the same payload. If the extension is no longer loaded when restore runs, the tab renders a placeholder until the extension comes back.

## Limits and gotchas

- One `WKWebView` per tab instance. Tabs do not share JavaScript context. To share state across tabs, route through the extension subprocess.
- The page cannot navigate to external URLs (`http://`, `https://`, `file://`). Only `muxy-ext://` and `about:` are allowed. Open external links yourself with `muxy.tabs.open()` for an editor tab or a future link-handling API.
- `runScript` command actions are accepted by the manifest validator but not yet executed — selecting one is a no-op until inline-script support lands in a follow-up.
