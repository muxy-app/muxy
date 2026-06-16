# Extension Tabs

A tab type lets an extension render its own HTML/CSS/JS as a full tab inside Muxy. Each opened tab is a separate `WKWebView`; tabs do not share a JavaScript context. The page talks to Muxy through the injected [`window.muxy`](#windowmuxy) bridge, which enforces the same [permissions](permissions.md) as everything else.

## Declaring a tab type

```json
{
  "name": "pr-tools",
  "version": "0.1.0",
  "permissions": ["tabs:write", "notifications:write"],
  "tabTypes": [
    {
      "id": "pr-viewer",
      "title": "PR Viewer",
      "entry": "index.html",
      "defaultData": { "mode": "compact" }
    }
  ],
  "commands": [
    { "id": "open-pr", "title": "Open PR…", "action": { "kind": "openTab", "tabType": "pr-viewer" } }
  ]
}
```

### Fields

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `id` | string | yes | Stable per extension. Referenced from `openTab` commands and from `muxy.tabs.open()`. |
| `title` | string | yes | Default tab title, until the page sets its own. |
| `entry` | string | yes | HTML path relative to the build output — any layout works (e.g. root `index.html`); not a fixed `tabs/` folder. Must resolve inside the extension directory (no `..` traversal). |
| `defaultData` | object | no | JSON merged into `window.muxy.data` when no explicit data is passed at open time. |

The page loads at `muxy-ext://<extensionID>/<entry>` and references its own files with relative paths; the scheme is scoped to that one extension's directory.

## Topbar (recommended)

A tab fills its whole region with one webview, so the page renders all of its own chrome. Extension tabs open with a thin **topbar** at the top — a horizontal bar holding the title on the left and controls on the right. **Render a matching topbar at the top of your page so your tab feels native; split panes line up only when every tab uses the same bar.**

The bar's height tracks the user's interface scale (Settings → Interface), so don't hardcode it — Muxy injects it as the `--muxy-topbar-height` CSS variable, updated live when the scale or theme changes. Use it together with the theme variables so the bar matches the app exactly:

```css
.topbar {
  box-sizing: content-box;
  height: var(--muxy-topbar-height);
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 12px;
  background: var(--muxy-background);
  border-bottom: 1px solid var(--muxy-border);
  flex: 0 0 auto;
}
.topbar .title { color: var(--muxy-foreground); font-weight: 600; }
.topbar .actions { margin-left: auto; display: flex; gap: 4px; }
```

`--muxy-topbar-height` is the bar's content height; native tabs draw their 1px divider *below* it, so keep `box-sizing: content-box` on `.topbar` (the border adds beneath the height rather than eating into it) for the divider to land on the same line as adjacent native tabs.

```html
<body>
  <header class="topbar">
    <span class="title">PR Viewer</span>
    <span class="actions">
      <button id="refresh">Refresh</button>
    </span>
  </header>
  <main class="content"><!-- your tab body --></main>
</body>
```

Because the topbar is your own HTML, you control its contents — put a title and icon on the left and as many action buttons or icons on either side as you need. To render edge-to-edge content instead (a canvas, a custom layout that owns the whole tab), simply omit the topbar; nothing in Muxy forces one.

See [theming](README.md) and the SKILL for the full `--muxy-*` variable list and copy-paste CSS.

## window.muxy

Muxy injects `window.muxy` before the page's scripts run. Most methods return a `Promise` and require their matching manifest permission — an unauthorized call rejects with `permission denied (<permission>)`. The subscription helpers (`onDataChange`, `onThemeChange`, `onFocus`, `events.subscribe`) instead return a synchronous unsubscribe function.

```ts
window.muxy = {
  extensionID: string,
  tabInstanceID: string,
  data: object | null,                 // payload the tab was opened with (or defaultData)
  onDataChange(callback): unsubscribe, // fires when a singleton tab is reopened with new data
  theme: object,                       // current --muxy-* theme values
  onThemeChange(callback): unsubscribe,
  focused: boolean,                    // whether this surface is the active, focused one
  onFocus(callback): unsubscribe,      // fires when focus is gained or lost — autofocus your editor here

  notifications: {
    notify({ title, body?, paneID? }): Promise<void>,   // requires notifications:write
  },
  toast({ title, body?, paneID? }): Promise<void>,        // same as notifications.notify

  dialog: {                                               // native sheets — see dialogs.md
    confirm(opts): Promise<string | null>,                // resolves the chosen button label, null on cancel
    alert(opts): Promise<void>,
  },
  modal: { open(opts): Promise<Item | null> },            // searchable picker — see modal.md

  tabs: {
    open(request): Promise<void>,       // see "Opening another tab"
    list(): Promise<TabInfo[]>,
    switchTo(idOrIndex): Promise<void>,
    new(): Promise<string | null>,
    next(): Promise<void>,
    previous(): Promise<void>,
    setTitle(title): Promise<void>,     // retitle this tab; "" resets to the manifest default
    setIcon(icon): Promise<void>,       // set this tab's icon; null resets to the default
  },

  panes: {
    list(): Promise<PaneInfo[]>,
    send(paneID, text): Promise<void>,
    sendKeys(paneID, key): Promise<void>,
    readScreen(paneID, lines?): Promise<string>,
    close(paneID): Promise<void>,
    rename(paneID, title): Promise<void>,
  },

  projects:  { list(), switchTo(identifier), delete(identifier) },  // delete() needs projects:delete + consent
  worktrees: { list(project?), switchTo(identifier, project?), refresh(project?) },
  panels:    { open(id, data?), toggle(id, data?), close(id) },  // panels:write — see panels.md
  popover:   { close(), resize(width, height) },                // panels:write — see popovers.md
  topbar:    { set(opts), show(id), hide(id) },                 // panels:write — see topbar.md
  statusbar: { set(opts), show(id), hide(id) },                 // panels:write — see statusbar.md
  git:       { status, diff, log, branches, commit, push, /* … */ pr: {}, branch: {}, worktree: {}, tag: {} }, // see git.md
  files:     { list, read, stat, write, mkdir, rename, move, delete },  // see files.md
  http:      { fetch(url, options?): Promise<HTTPResult> },     // no CORS — see http.md
  events:    {
    subscribe(name, callback): unsubscribe,
    emit(name: `extension.${string}`, payload?): Promise<void>,
  },
  exec(argv: string[], options?): Promise<ExecResult>,
  exec(options: { shell: string, ... }): Promise<ExecResult>,
}

interface ExecResult {
  stdout: string;
  stderr: string;
  exitCode: number;
  timedOut: boolean;
}
```

### Opening another tab

`tabs.open` accepts two kinds: `terminal` and `extensionWebView` (with a target `extension`). It is available from tabs, panels, popovers, `runScript` commands, and background scripts; non-webview callers open into the active workspace and reject when Muxy cannot identify one.

```js
await muxy.tabs.open({ kind: 'terminal' });
await muxy.tabs.open({
  kind: 'extensionWebView',
  extension: { id: 'pr-tools', tabType: 'pr-viewer', data: { prNumber: 42 } },
});
```

`extensionWebView` requires the target extension to be loaded and the named tab type to exist.

A `terminal` tab accepts two optional fields:

```js
await muxy.tabs.open({ kind: 'terminal', directory: 'packages/app', command: 'npm run dev' });
```

| Field | Type | Notes |
| --- | --- | --- |
| `directory` | string | Opens the terminal in this folder. Resolved relative to the active worktree root and **must stay inside it** — paths that escape via `..` or symlinks are rejected, and the folder must already exist. Needs only `tabs:write`. |
| `command` | string | A startup command auto-run in the new terminal. It runs interactively and the tab stays open after it exits. Because auto-running a command is sensitive, this additionally triggers a one-time runtime consent prompt on top of `tabs:write`. |

The two are independent: `directory` alone needs no extra consent; `command` alone (no `directory`) opens in the default location; both together open in `directory` and run `command`. These fields are how an extension drives its own session restore — pair them with the enriched [`tab.*` events](events.md) to record a session, then recreate each terminal tab. Extension-webview tabs are restorable the same way: pass back the recorded `extensionID`/`tabTypeID` as `extension.id`/`extension.tabType`, and the `data` parsed from the tab event's JSON `data` string as `extension.data`.

By default every `open` creates a new tab. Pass `singleton: true` to keep one tab per tab type instead — if a tab of that type is already open, Muxy focuses it and pushes the new `data` into the live page rather than duplicating it. The page receives the new payload through `muxy.onDataChange`:

```js
await muxy.tabs.open({
  kind: 'extensionWebView',
  extension: { id: 'pr-tools', tabType: 'pr-viewer', singleton: true, data: { prNumber: 42 } },
});

muxy.onDataChange((data) => render(data));
```

### Reacting to focus

A page learns when its surface becomes the active, focused one through `muxy.onFocus`. Use it to move keyboard focus into your own UI — autofocus an editor or input the moment the tab is opened or switched back to:

```js
const editor = document.querySelector('textarea');
muxy.onFocus((focused) => {
  if (focused) editor.focus();
});
```

`muxy.focused` reads the current state synchronously. The callback fires only on a change — gaining focus when its tab is opened or selected, losing it when another tab takes over. Panels and popovers count as focused while they are shown.

### Setting the tab title and icon at runtime

A tab can rename itself and change its tab-bar icon live — useful when the page reflects changing state (a file editor showing the open file, a build tool showing pass/fail). Both apply to the calling page's own tab and take effect immediately, no reopen.

```js
await muxy.tabs.setTitle('App.swift');
await muxy.tabs.setIcon({ symbol: 'swift' });   // SF Symbol
await muxy.tabs.setIcon({ svg: 'icons/file.svg' }); // bundled SVG, template-rendered
```

| Call | Notes |
| --- | --- |
| `setTitle(title)` | New tab-bar title. An empty/whitespace string resets to the manifest `tabType.title`. |
| `setIcon(icon)` | `"<sf-symbol>"`, `{ symbol }`, or `{ svg }` (path inside the extension). `null` resets to the default extension icon. |

Both need `tabs:write`. Overrides are **runtime-only** — they live while the tab is open and reset to the manifest defaults on app restart, so set them again from your page on load. A page can only customize its own tab.

### Running shell commands

`exec` requires `commands:exec`. Use the argv form to avoid a shell (no quoting concerns) or the `{ shell }` form for pipes and expansion.

```js
const { stdout, exitCode } = await muxy.exec(['git', 'diff', '--name-only']);
const counted = await muxy.exec({ shell: 'git diff | wc -l' });
await muxy.exec(['ls'], { cwd: '~', timeoutMs: 5000 });
```

- Default cwd is the active worktree; override with `options.cwd` (`~` expands).
- Default timeout is 30 s. On timeout the child gets `SIGTERM`, then `SIGKILL` 2 s later, and the Promise resolves with `timedOut: true`.
- Combined output is capped at 10 MB; beyond that it resolves with `truncated: true` and the captured prefix.
- `PATH` is taken from the user's login shell at startup, so `git`, `npm`, etc. resolve without absolute paths.

### Subscribing to workspace events

```js
const unsubscribe = muxy.events.subscribe('tab.focused', (p) => console.log(p.tabID));
unsubscribe();
```

The event must be listed in the manifest `events` array (a `command.<id>` event of the same extension is auto-allowed); otherwise the subscribe rejects. Subscriptions drop automatically on page reload, tab close, and extension disable/reload.

For webview-to-background coordination, use extension-local events. Names must start with `extension.`, do not go in the manifest, and are scoped to the same extension:

```js
await muxy.events.emit('extension.editor.saved', { path: 'Sources/App.swift' });
```

An extension-local emit requires the extension's `background.js` to be running. The background script can subscribe to the same name and can emit `extension.*` events back to open tabs, panels, and popovers.

## Persistence

Workspace restore persists each tab's `extensionID`, `tabTypeID`, and `data`, so it reopens with the same payload. If the extension isn't loaded when restore runs, the tab shows a placeholder until it returns.

## Logging

`console.log` / `warn` / `error`, uncaught errors, and unhandled rejections are mirrored to the extension's [log file](logs.md).

## Limits

- One `WKWebView` per tab instance; tabs do not share state. Coordinate shared state through your background script with `extension.*` events.
- Pages can only navigate within `muxy-ext://` and `about:` — no `http`/`https`/`file`. Open external content via `muxy.tabs.open()`.
- Background scripts only expose `muxy.tabs.open`; tab listing, switching, and customization remain page and `runScript` capabilities.
- For command logic with no UI, use a [`runScript`](scripts.md) command action instead of a hidden tab.
