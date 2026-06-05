# Manifest

Every extension is an npm + [Vite](https://vitejs.dev) project. Its manifest is the `muxy` object inside the `package.json` at the root of its directory.

Identity (`name` and `version`) lives at the **top level** of `package.json` — npm's single source of truth. **Every other manifest field** (`description`, `background`, `events`, `permissions`, `tabTypes`, `panels`, `popovers`, `commands`, `topbarItems`, `statusBarItems`, `settings`, `remoteMethods`, `marketplace`) lives under the `muxy` key.

`package.json` must also declare a `build` script. The publishing pipeline runs `npm run build` (Vite) and ships the build output directory, `dist/`. The app installs and reads from `dist/`, so every entry/asset path inside `muxy` (popover/tab `entry`, `background`, marketplace `icon`/`screenshots`) resolves against the build output, not your source tree.

There is **no fixed folder layout**. Every `entry`/`background`/icon path is an arbitrary relative path inside the build output — point it wherever your build emits the file. The vanilla starter kit emits its panel to `panel/index.html`, but any layout works equally. The only two names Muxy fixes are `package.json` (the manifest) and `dist/` (the build output it ships and reads).

```json
{
  "name": "hello",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": { "dev": "vite", "build": "vite build" },
  "devDependencies": { "vite": "^5.0.0" },
  "muxy": {
    "$schema": "https://raw.githubusercontent.com/muxy-app/muxy/main/docs/extensions/schema/manifest.schema.json",
    "description": "Subscribes to events and exposes a palette command",
    "background": "background.js",
    "permissions": ["panes:read", "tabs:read", "notifications:write"],
    "events": ["pane.created", "tab.focused", "notification.posted"],
    "commands": [
      { "id": "ping", "title": "Hello: Ping", "subtitle": "Demo command" }
    ]
  }
}
```

## Top-level fields

These are standard npm fields read directly from `package.json`. Only `name` and `version` carry Muxy meaning; the rest configure the build.

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `name` | string | yes | Letters, digits, `-`, `_`, `.` only (no leading dot). Must match the directory name. Used as the extension ID. |
| `version` | string | yes | Semver. A published `name@version` is immutable; bump for any change. Shown in Settings. |
| `scripts` | object | yes | npm scripts. Must include a `build` script — the publishing pipeline runs it and ships the resulting `dist/`. |

## `muxy` fields

All Muxy-specific manifest fields live under the `muxy` object.

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `description` | string | no | One-line description shown in Settings. |
| `background` | string | no | Path (relative to the build output) to a JavaScript file that must resolve inside `dist/`. Declare it only to receive pushed [events](events.md), coordinate webviews with `extension.*` events, or run background shell commands; Muxy runs it in a long-lived host process. Command, topbar, status-bar, tab, and `runScript` extensions need none. |
| `permissions` | string[] | no | See [Permissions](permissions.md). Verbs not listed are rejected. Defaults to empty. |
| `events` | string[] | no | Workspace events the extension may subscribe to. Local `extension.*` events are not declared here. See [Events](events.md). Defaults to empty. |
| `commands` | object[] | no | Palette commands to register. See [Palette Commands](palette-commands.md). |
| `tabTypes` | object[] | no | Webview tab types the extension exposes. See [Tabs](tabs.md). |
| `panels` | object[] | no | Dockable/floating webview panels. See [Panels](panels.md). |
| `popovers` | object[] | no | Transient webview popovers anchored to a topbar/status-bar item. See [Popovers](popovers.md). |
| `topbarItems` | object[] | no | Icons attached to the tab strip. See [Topbar](topbar.md). |
| `statusBarItems` | object[] | no | Icons attached to the footer status bar. See [Status Bar](statusbar.md). |
| `settings` | object[] | no | Typed settings shown in the Settings sidebar. See [Settings](settings.md). |
| `remoteMethods` | object[] | no | Named API methods served to the mobile app. Requires `remote:serve`. See [Remote Methods](remote-methods.md). |
| `marketplace` | object | no | Listing metadata (icon, screenshots, author, categories) used by the marketplace. Ignored by the app loader. |

Extensions are enabled by default after loading. The Settings → Extensions toggle is persisted in `UserDefaults` under `muxy.ext.enabled.<extension-id>` and survives launches. A legacy `enabled` manifest field is no longer part of the schema; if present with no user override, it is migrated into that UserDefaults entry on first load and otherwise ignored.

## Icons

Topbar and status-bar items accept an `icon` field in one of two forms:

```json
{ "icon": { "symbol": "puzzlepiece.extension" } }
{ "icon": { "svg": "assets/badge.svg" } }
```

A bare string (`"icon": "puzzlepiece.extension"`) is shorthand for `{ "symbol": ... }`.

- **`symbol`** — any SF Symbol name. Tinted with the chrome's foreground color.
- **`svg`** — a path relative to the build output to a `.svg` file. The file must exist in `dist/` at load time, must not escape the extension directory, and must be at most 256 KiB. Rendered as a template image, so fills/strokes using `currentColor` (or a single solid color) pick up the chrome tint.

## Loader behaviour

The publishing pipeline runs `npm run build` and ships the build output (`dist/`); the app installs that into `~/.config/muxy/extensions/<name>/`. `ExtensionStore` walks `~/.config/muxy/extensions/*/package.json` at app start. For each one it decodes the top-level `name`/`version` and the `muxy` object, validates `name` against the allowed character set and against the directory name, verifies the `background` file resolves inside the build output (if declared), and refuses duplicate names. Failures appear in **Settings → Extensions → Load Errors**; the app does not retry until you click **Reload Extensions** or restart.

## Background script environment

A `background` script never speaks a wire protocol. Muxy handles the socket, identity token, and handshake; authors only use the `muxy` global it injects:

- `muxy.extensionID` — the extension's `name`.
- `muxy.events.subscribe(name, handler)` / `unsubscribe` — receive declared workspace [events](events.md) and same-extension `extension.*` events.
- `muxy.events.emit(name, payload?)` — send a same-extension `extension.*` event to open tabs, panels, and popovers.
- `muxy.exec(argv[, options])` — run a shell command (needs `commands:exec`).
- `console.log` / `console.warn` / `console.error` — written to the extension log.

The richer state/mutation API (`tabs`, `panes`, `projects`, `worktrees`, etc.) is available only to tab/panel/popover pages via `window.muxy`, not to the background script.
