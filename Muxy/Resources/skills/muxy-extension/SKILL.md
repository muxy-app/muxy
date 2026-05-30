---
name: muxy-extension
description: Use when authoring or modifying a Muxy extension. Covers manifest fields, the two extension surfaces (the `background.js` host and the in-page `window.muxy` bridge), the permission model, theme adaptation, and end-to-end examples drawn from the reference extension.
---

# Muxy Extension Author Guide

Muxy extensions live in `~/.config/muxy/extensions/<name>/` and load when Muxy starts. Each extension is a directory containing a `manifest.json`. Optional resources (HTML tabs, a background script, icons, assets) live alongside.

## The two surfaces — pick the right one

A Muxy extension has two independent surfaces. Most of getting an extension right is choosing the correct one for each piece of work:

- **UI pages (in-process).** Tabs, panels, and popovers are HTML/CSS/JS rendered in a `WKWebView`. Their page scripts get the **full `window.muxy` API** — `tabs`, `panes`, `projects`, `worktrees`, `events`, `exec`, `toast`, `panels`, `popover`, plus `data`, `theme`, and `tabInstanceID`. Use a page whenever you need to *show* something.
- **Background script (`background.js`, out-of-process).** Optional. Muxy runs it in a long-lived host process and gives it a **small `muxy` global** — only `extensionID`, `events.subscribe`/`unsubscribe`, `exec`, and `console.*`. Use it only to **react to pushed events** or **run shell commands on a schedule/in the background**, independent of any open tab. Most extensions don't need one.

Rule of thumb: long-lived, event-driven, or headless work → `background.js`. Anything the user looks at → a UI page. Don't open a hidden tab just to run logic — use a [`runScript`](#run-script-commands-javascriptcore-sandbox) command instead.

## When to use this skill

Use this skill when:

- Writing a new Muxy extension (manifest, background script, tab/panel/popover UI).
- Adding a command, topbar item, status-bar item, settings entry, tab type, panel, or popover.
- Styling an extension page so it adapts to the user's current Muxy theme.
- Reading Muxy state (panes, tabs, projects, worktrees) or running shell commands from a page.
- Reacting to workspace events from a `background.js` script.

## Project layout

A typical extension looks like this:

```
my-extension/
├── manifest.json           # required
├── background.js           # optional background script (events / muxy.exec)
├── CLAUDE.md               # author guide for this extension
├── AGENTS.md → CLAUDE.md   # symlink for non-Claude agents
├── .gitignore
├── tabs/
│   ├── playground.html
│   ├── playground.css
│   └── playground.js
├── scripts/
│   └── do-something.js     # invoked via { "kind": "runScript" }
└── assets/
    └── icon.svg            # used by topbar/status-bar items
```

Every relative path in `manifest.json` is resolved against the extension directory and rejected if it escapes the directory.

## Manifest

The full reference manifest, taken from the bundled demo extension:

```json
{
  "name": "demo",
  "version": "0.2.0",
  "description": "Reference extension: playground tab, runScript command, topbar icon, status bar items, and settings.",
  "background": "background.js",
  "permissions": [
    "tabs:read", "tabs:write",
    "panes:read", "panes:write",
    "projects:read", "projects:write",
    "worktrees:read", "worktrees:write",
    "notifications:write",
    "commands:run-script",
    "commands:exec"
  ],
  "tabTypes": [
    { "id": "playground", "title": "Muxy API Playground", "entry": "tabs/playground.html" },
    { "id": "dashboard",  "title": "Git Dashboard",       "entry": "tabs/dashboard.html"  }
  ],
  "commands": [
    {
      "id": "open-playground",
      "title": "Demo: Open Playground",
      "action": { "kind": "openTab", "tabType": "playground" }
    },
    {
      "id": "run-script",
      "title": "Demo: Open Git Dashboard",
      "action": { "kind": "runScript", "script": "scripts/git-status.js" }
    }
  ],
  "topbarItems": [
    {
      "id": "playground",
      "icon": { "svg": "assets/playground.svg" },
      "tooltip": "Open Demo Playground",
      "command": "open-playground"
    }
  ],
  "statusBarItems": [
    {
      "id": "ticker",
      "icon": { "symbol": "leaf.fill" },
      "text": "ready",
      "tooltip": "Demo ticker (left)",
      "side": "left",
      "command": "open-playground"
    },
    {
      "id": "dashboard",
      "icon": { "symbol": "chart.bar.fill" },
      "tooltip": "Open Git Dashboard",
      "side": "right",
      "command": "run-script"
    }
  ],
  "settings": [
    {
      "key": "refreshSeconds",
      "title": "Refresh Interval (s)",
      "description": "How often to update the left status bar ticker.",
      "type": "number",
      "defaultValue": 5
    }
  ]
}
```

Field-by-field:

- `name` — required. Alphanumerics, dash, underscore, dot only. Must match the directory name.
- `version` — required semver string.
- `description` — optional one-line summary shown in the Extensions modal.
- `background` — optional relative path to a JavaScript file. When present it must exist inside the extension directory; Muxy runs it in a long-lived host process for the lifetime of the extension. Provide it only to receive pushed events (`muxy.events.subscribe`) or run background shell commands (`muxy.exec`) — command, topbar, status bar, tab, and `runScript` extensions need none, and omitting it means Muxy keeps no resident process.
- `permissions` — array of permission strings. Declare only what the background script or tabs actually use.
- `events` — array of event names this extension subscribes to (for example `pane.created`, `tab.focused`, `pane.closed`). Command events (`command.<id>`) are auto-allowed.
- `tabTypes` — declares HTML pages renderable as tabs.
- `panels` — declares HTML pages renderable as dockable/floating panels (`position`: `right`|`bottom`, `mode`: `floating`|`pinned`, optional `icon`/`title`/`hiddenControls`). Requires `panels:write` to open/close at runtime. One pinned and one floating panel per position; opening another in that slot replaces it.
- `popovers` — declares HTML pages renderable as transient popovers anchored to a topbar/status-bar item (`entry` required; optional `title`, `width`, `height` defaulting to 320×360). Frameless, auto-dismiss on outside click, at most one open at a time. Opened via an `openPopover` command bound to a topbar/status-bar item; the page sizes itself with `muxy.popover.resize()` (needs `panels:write`).
- `commands` — palette commands. Each command's `action.kind` is `event` (default — fires `command.<id>`), `openTab`, `togglePanel`, `openPopover`, or `runScript`.
- `topbarItems` / `statusBarItems` — UI hooks bound to a command. `icon` is either `{ "symbol": "<sf-symbol>" }` or `{ "svg": "<relative/path.svg>" }`.
- `settings` — user-visible settings (`string` | `bool` | `number`) editable in the Extensions modal. Values persist per extension. (Reading/writing them programmatically is a socket verb used by the `muxy` CLI; it is not exposed on the background `muxy` global yet.)

Common load failures: a declared `background` script that is missing or escapes the extension directory, tab/panel/popover entry escapes the extension directory, a command references an unknown `tabType`, `panel`, or `popover`, a topbar or status-bar item references an unknown command. Failures appear in the Extensions modal under "Load Errors".

## Permissions reference

Permissions are gated server-side. Requests without the matching permission fail.

| Permission | Enables |
| --- | --- |
| `panes:read` | `panes.list`, `panes.readScreen` |
| `panes:write` | `panes.send`, `panes.sendKeys`, `panes.close`, `panes.rename` |
| `tabs:read` | `tabs.list` |
| `tabs:write` | `tabs.open`, `tabs.switch`, `tabs.new`, `tabs.next`, `tabs.previous` |
| `projects:read` | `projects.list` |
| `projects:write` | `projects.switch` |
| `worktrees:read` | `worktrees.list` |
| `worktrees:write` | `worktrees.switch`, `worktrees.refresh` |
| `notifications:write` | `toast` |
| `panels:write` | `panel.open`, `panel.toggle`, `panel.close`, `popover.resize`, `popover.close` |
| `commands:run-script` | `runScript` commands |
| `commands:exec` | `muxy.exec` (always prompts the user the first time) |

Principle: least privilege. Add a permission only when adding the call that requires it.

## Background script

The `background` script is optional. Manifest UI (palette commands, topbar items, status-bar items, tab types) and `runScript` commands all work without one. Add a background script only when the extension must **receive pushed events** or **run shell commands on its own** (not in response to a tab). Muxy runs the script in a long-lived host process for the lifetime of the extension; without one there is no resident process.

The host exposes a small `muxy` global to `background.js`:

- `muxy.extensionID` — the extension's `name`.
- `muxy.events.subscribe(name, handler)` / `muxy.events.unsubscribe(name, handler)` — receive workspace events declared in `events`. `handler(payload)` is called with the event payload object.
- `muxy.exec(argv[, options])` / `muxy.exec(options)` — run a shell command (needs `commands:exec`; prompts the user the first time, then honours remembered allow/deny rules). Returns `{ stdout, stderr, exitCode, timedOut, truncated }`.
- `console.log` / `console.warn` / `console.error` — written to the extension log.

The richer state/mutation API (`muxy.tabs`, `muxy.panes`, `muxy.projects`, `muxy.worktrees`) is available to **tab/panel/popover pages** via `window.muxy`, not to the background script.

### Example `background.js` (subscribe to an event, run a command)

```js
muxy.events.subscribe('pane.created', async (payload) => {
  console.log('pane created', payload.paneID);
  const result = await muxy.exec(['git', 'status', '--short']);
  console.log(result.stdout);
});
```

The first `muxy.exec` for a given command prompts the user in the main window. If they allow-and-remember it, matching commands run without prompting; if they deny, the call rejects with "response not allowed".

## In-tab bridge (`window.muxy`)

When a tab type renders an HTML page, Muxy injects a `window.muxy` object before the page scripts run. Use it to read Muxy state, open tabs, mutate panes, subscribe to events, and read the current theme.

### Bootstrap (read context and theme)

```js
console.log('running as', muxy.extensionID, 'in tab', muxy.tabInstanceID);
console.log('initial data payload:', muxy.data);
console.log('current theme:', muxy.theme);

muxy.onThemeChange((theme) => {
  // Theme changed (user toggled light/dark or accent). CSS variables
  // (--muxy-background, --muxy-accent, ...) are already updated on
  // document.documentElement — this hook is for JS-driven re-renders.
  console.log('theme changed to', theme.colorScheme, theme.accent);
});
```

### Read Muxy state

```js
const tabs       = await muxy.tabs.list();
const panes      = await muxy.panes.list();
const projects   = await muxy.projects.list();
const worktrees  = await muxy.worktrees.list();

const activeProject = projects.find((p) => p.isActive);
```

### Open / switch / mutate tabs

```js
await muxy.tabs.new();                                  // new terminal tab
await muxy.tabs.next();                                 // cycle forward
await muxy.tabs.switchTo(0);                            // by index

await muxy.tabs.open({ kind: 'terminal' });
await muxy.tabs.open({ kind: 'vcs' });
await muxy.tabs.open({ kind: 'editor', filePath: '/abs/path/README.md' });

// Open another instance of this extension's tab, with a custom data payload.
await muxy.tabs.open({
  kind: 'extensionWebView',
  extension: {
    id: muxy.extensionID,
    tabType: 'dashboard',
    data: { source: 'self', when: new Date().toISOString() },
  },
});
```

### Open / close panels

Requires `panels:write`. The panel id must be declared under `panels` in the manifest.

```js
await muxy.panels.open('dashboard');                 // open (or move) the panel
await muxy.panels.toggle('dashboard');               // open if closed, close if open
await muxy.panels.open('dashboard', { tab: 'logs' }); // override defaultData for this instance
await muxy.panels.close('dashboard');
```

### Size / close a popover

Available inside a popover page. Requires `panels:write`. A popover is opened by the user from its anchoring topbar/status-bar item (there is no `open` from JS) — the page only sizes itself to its content and can dismiss itself.

```js
// Fit the popover to its content once laid out:
window.addEventListener('load', () =>
  muxy.popover.resize(
    document.documentElement.scrollWidth,
    document.documentElement.scrollHeight
  )
);

await muxy.popover.close(); // dismiss self
```

### Drive terminal panes

```js
const [pane] = await muxy.panes.list();
await muxy.panes.send(pane.id, 'echo hi\n');           // write text
await muxy.panes.sendKeys(pane.id, 'Enter');           // press a key
await muxy.panes.rename(pane.id, 'Renamed');
const buffer = await muxy.panes.readScreen(pane.id, 5); // last 5 lines
```

### Run shell

```js
// Simple argv (no shell parsing):
const result = await muxy.exec(['git', 'status', '--short']);
// { exitCode, stdout, stderr, timedOut }

// Shell string (uses /bin/sh -c):
await muxy.exec({ shell: 'git diff | wc -l' });

// With working dir and a hard timeout:
await muxy.exec(['ls', '-1'], { cwd: '~' });
await muxy.exec(['sleep', '5'], { timeoutMs: 500 }); // timedOut: true
```

`muxy.exec` requires `commands:exec` and prompts the user the first time. Users can save allow/deny rules per command.

### Subscribe to live events

```js
const off = muxy.events.subscribe('pane.created', (payload) => {
  console.log('new pane:', payload);
});

// Stop listening:
off();
```

Only events declared in `manifest.events` (or auto-allowed command events) reach the callback.

### Notifications

```js
await muxy.toast({ title: 'Done', body: 'Build finished in 3.2s' });
```

## Run-script commands (JavaScriptCore sandbox)

A command with `{ "kind": "runScript", "script": "scripts/x.js" }` runs in a per-extension JavaScriptCore context (no DOM, no Node) that exposes the same `muxy.*` surface as tabs, plus `console.log`. Calls are **synchronous** here (no `await`). Use this for one-shot tasks that compute data and then open a tab to display it — it's the right tool for headless logic triggered from the palette, instead of opening a hidden tab.

Example — `scripts/git-status.js` from the demo extension:

```js
function run(argv) {
  const result = muxy.exec(argv); // synchronous in scripts
  return result.exitCode === 0 ? result.stdout.trim() : '';
}

const branch       = run(['git', 'rev-parse', '--abbrev-ref', 'HEAD']);
const totalCommits = Number(run(['git', 'rev-list', '--count', 'HEAD'])) || 0;

muxy.tabs.open({
  kind: 'extensionWebView',
  extension: {
    id: muxy.extensionID,
    tabType: 'dashboard',
    data: { branch, totalCommits, generatedAt: new Date().toISOString() },
  },
});
```

Receive the payload in the tab as `muxy.data`.

## Theming — adapt to the user's current Muxy theme

**Do not hardcode colors. Adopting the app theme is the best practice — and it is required for popovers,** which sit directly against the app chrome and look broken if they don't match. Muxy supports paired light/dark themes and a user-selected accent color. Every extension webview — tab, panel, **and popover** — inherits the same CSS custom properties on `document.documentElement` that match the live theme. They update automatically when the user changes theme, and the rules below apply identically to all three surfaces.

**Popovers: leave the page background transparent.** The webview is presented over the native macOS popover material (translucent vibrancy that is already light/dark-aware), and its backing is non-opaque. Set `body { background: transparent; }` — do **not** paint `--muxy-background` on the popover body — so the system material shows through and the popover matches macOS automatically. Foreground text, accents, and translucent `--muxy-surface` chips/buttons still use the theme variables as usual. (Tabs and panels fill their whole region, so they *do* paint `--muxy-background` on the body.)

### Available CSS variables

| Variable | Use for |
| --- | --- |
| `--muxy-background` | Page background |
| `--muxy-foreground` | Primary text |
| `--muxy-foreground-muted` | Secondary text, labels, captions |
| `--muxy-surface` | Cards, buttons, code blocks, input backgrounds |
| `--muxy-border` | 1px borders, dividers |
| `--muxy-hover` | Hover state for buttons / rows |
| `--muxy-accent` | Primary action color, links, focus rings |
| `--muxy-accent-soft` | Translucent accent for highlights, badges |
| `--muxy-diff-add` | Added lines, success states |
| `--muxy-diff-remove` | Removed lines, error states |
| `--muxy-diff-hunk` | Hunk headers in diffs |
| `--muxy-color-scheme` | Mirrors `document.documentElement.style.colorScheme` (`light` / `dark`) |

### Best-practice CSS — copy as a starting point

```css
* { box-sizing: border-box; }

body {
  margin: 0;
  padding: 16px;
  font: 13px -apple-system, "SF Pro", system-ui, sans-serif;
  background: var(--muxy-background);
  color: var(--muxy-foreground);
}

h2 {
  font-size: 11px;
  margin: 16px 0 6px;
  color: var(--muxy-foreground-muted);
  text-transform: uppercase;
  letter-spacing: 0.6px;
}

button {
  background: var(--muxy-surface);
  color: var(--muxy-foreground);
  border: 1px solid var(--muxy-border);
  border-radius: 5px;
  padding: 6px 10px;
  font: inherit;
  cursor: pointer;
}
button:hover  { background: var(--muxy-hover); border-color: var(--muxy-accent); }
button:active { transform: translateY(1px); }

.card {
  background: var(--muxy-surface);
  border: 1px solid var(--muxy-border);
  border-radius: 8px;
  padding: 14px 16px;
}

.badge {
  font-family: "SF Mono", Menlo, monospace;
  font-size: 12px;
  padding: 2px 8px;
  border-radius: 10px;
  background: var(--muxy-surface);
  color: var(--muxy-accent);
  border: 1px solid var(--muxy-border);
}

pre, code {
  font-family: "SF Mono", Menlo, monospace;
  background: var(--muxy-surface);
  color: var(--muxy-foreground);
}

.diff-add    { color: var(--muxy-diff-add); }
.diff-remove { color: var(--muxy-diff-remove); }
.diff-hunk   { color: var(--muxy-diff-hunk); }
```

### Theming rules

1. **No hex literals for UI chrome.** Use `var(--muxy-…)` everywhere. The only exception is decorative art that is meant to be theme-independent.
2. **Treat `--muxy-accent` as the only saturated color.** Use it sparingly — for the primary action, focus rings, key numbers — so it stays distinctive.
3. **Use `--muxy-surface` for elevation.** Cards, code blocks, inputs, and buttons share one surface color; depth comes from `--muxy-border` and `--muxy-hover`, not from new colors.
4. **Make hover states obvious.** `background: var(--muxy-hover); border-color: var(--muxy-accent);` is the standard pattern.
5. **Light-on-accent text** — when filling a chip or pill with `var(--muxy-accent)`, set its text color to `var(--muxy-background)` so it stays legible in both light and dark.
6. **Respect `prefers-reduced-motion`.** Muxy users opt into Reduce Motion at the OS level; avoid long transitions, large translations, or autoplay animations.
7. **Don't sniff `colorScheme` to pick colors.** Variables already invert. Only branch on `muxy.theme.colorScheme` for things variables can't express (for example, swapping a logo image).
8. **JS-driven re-renders must re-read the theme.** Use `muxy.onThemeChange(theme => …)` to redraw canvas/SVG that doesn't pick up CSS variables automatically.

### Theming example (JS-side)

This is the pattern from the demo playground tab:

```js
const badge = document.createElement('span');
badge.style.cssText =
  'padding:1px 6px;border-radius:3px;' +
  'background:var(--muxy-accent);color:var(--muxy-background);';
badge.textContent = `${muxy.theme.colorScheme} · ${muxy.theme.accent}`;
document.body.appendChild(badge);

muxy.onThemeChange((theme) => {
  badge.textContent = `${theme.colorScheme} · ${theme.accent}`;
});
```

## End-to-end example (minimal extension)

A complete "hello-world" extension that adds a palette command, a tab, and a theme-aware UI:

```
hello-world/
├── manifest.json
└── tabs/
    ├── index.html
    └── styles.css
```

This extension only opens a tab from a palette command, so it declares no background script and runs no resident process.

```json
// manifest.json
{
  "name": "hello-world",
  "version": "0.1.0",
  "description": "Minimal Muxy extension",
  "permissions": ["tabs:write"],
  "tabTypes": [
    { "id": "main", "title": "Hello", "entry": "tabs/index.html" }
  ],
  "commands": [
    {
      "id": "open",
      "title": "Hello World: Open",
      "action": { "kind": "openTab", "tabType": "main" }
    }
  ]
}
```

```html
<!-- tabs/index.html -->
<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <link rel="stylesheet" href="styles.css">
</head>
<body>
  <h1>Hello, <span id="who">world</span>!</h1>
  <button id="say">Toast</button>
  <script>
    document.getElementById('who').textContent = muxy.extensionID;
    document.getElementById('say').addEventListener('click', () =>
      muxy.toast({ title: 'Hello', body: `theme: ${muxy.theme.colorScheme}` })
    );
  </script>
</body>
</html>
```

```css
/* tabs/styles.css */
body {
  margin: 0; padding: 24px;
  font: 13px -apple-system, system-ui, sans-serif;
  background: var(--muxy-background);
  color: var(--muxy-foreground);
}
h1 { font-size: 18px; color: var(--muxy-accent); }
button {
  background: var(--muxy-surface);
  color: var(--muxy-foreground);
  border: 1px solid var(--muxy-border);
  border-radius: 5px;
  padding: 6px 10px;
}
button:hover { background: var(--muxy-hover); border-color: var(--muxy-accent); }
```

> Note: `muxy.toast` requires `notifications:write`. Add it to `permissions` if you use it.

## End-to-end example (popover)

A status-bar item that opens a self-sizing popover. The popover replaces what used to be a built-in popover (e.g. an AI-usage meter): a small, read-mostly surface anchored to its item.

```json
// manifest.json
{
  "name": "status-popover",
  "version": "0.1.0",
  "permissions": ["panels:write"],
  "popovers": [
    { "id": "summary", "title": "Summary", "entry": "popovers/summary.html", "width": 280, "height": 200 }
  ],
  "commands": [
    { "id": "open-summary", "title": "Status: Summary", "action": { "kind": "openPopover", "popover": "summary" } }
  ],
  "statusBarItems": [
    { "id": "summary", "icon": { "symbol": "gauge" }, "text": "Status", "side": "right", "command": "open-summary" }
  ]
}
```

```html
<!-- popovers/summary.html -->
<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <style>
    /* Popover: transparent body so the native macOS popover material shows through. */
    body { margin: 0; font: 13px -apple-system, system-ui, sans-serif;
           background: transparent; color: var(--muxy-foreground); }
    .box { padding: 16px; display: flex; flex-direction: column; gap: 10px; }
    .line { color: var(--muxy-foreground-muted); }
    button { font: inherit; padding: 6px 10px; border-radius: 6px;
             background: var(--muxy-surface); color: inherit;
             border: 1px solid var(--muxy-border); cursor: pointer; }
    button:hover { background: var(--muxy-hover); border-color: var(--muxy-accent); }
  </style>
</head>
<body>
  <div class="box">
    <strong>Summary</strong>
    <span class="line" id="line">loading…</span>
    <button onclick="muxy.popover.close()">Close</button>
  </div>
  <script>
    document.getElementById('line').textContent = `running as ${muxy.extensionID}`;
    window.addEventListener('load', () =>
      muxy.popover.resize(
        document.documentElement.scrollWidth,
        document.documentElement.scrollHeight
      )
    );
  </script>
</body>
</html>
```

The popover anchors to the status-bar item, opens/toggles when it is clicked, and dismisses on outside click. No background script is needed.

## Reload workflow

After editing `manifest.json`, scripts, tab HTML/CSS/JS, or the background script, click **Reload** in the Muxy Extensions modal. Muxy terminates the running process and re-validates the manifest. Tabs are not auto-refreshed — close and reopen them, or use `tabs.open` to get a fresh instance.

## Quick checklist before shipping

- [ ] `manifest.json` parses; any declared `background` script exists inside the extension directory.
- [ ] `permissions` declares only what is actually used.
- [ ] Every CSS rule for UI chrome uses `var(--muxy-…)`.
- [ ] `muxy.onThemeChange` is wired for any canvas/SVG/JS-rendered color.
- [ ] Hover and active states are visible in both light and dark themes.
- [ ] No hardcoded paths to `~/.config/muxy` from inside the extension — use `muxy.exec({ cwd: … })` or rely on the working directory Muxy sets.
- [ ] Event-driven work happens in the background script, not in tab JS, so closing a tab does not lose state. No background script unless events or background `muxy.exec` are needed.
