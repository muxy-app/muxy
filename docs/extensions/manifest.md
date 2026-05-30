# Manifest

Every extension declares itself in a `manifest.json` at the root of its directory.

```json
{
  "name": "hello",
  "version": "0.1.0",
  "description": "Subscribes to events and exposes a palette command",
  "background": "background.js",
  "permissions": ["panes:read", "tabs:read", "notifications:write"],
  "events": ["pane.created", "tab.focused", "notification.posted"],
  "commands": [
    { "id": "ping", "title": "Hello: Ping", "subtitle": "Demo command" }
  ],
  "aiProvider": null
}
```

## Fields

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `name` | string | yes | Letters, digits, `-`, `_`, `.` only. Should match the directory name. Used as the extension ID. |
| `version` | string | yes | Free-form. Shown in Settings. |
| `description` | string | no | One-line description shown in Settings. |
| `background` | string | no | Path (relative to manifest) to a JavaScript file that must resolve inside the extension directory. Declare it only to receive pushed [events](events.md) or run background shell commands; Muxy runs it in a long-lived host process. Command, topbar, status-bar, tab, and `runScript` extensions need none. |
| `permissions` | string[] | no | See [Permissions](permissions.md). Verbs not listed are rejected. Defaults to empty. |
| `events` | string[] | no | Events the extension may subscribe to. See [Events](events.md). Defaults to empty. |
| `commands` | object[] | no | Palette commands to register. See [Palette Commands](palette-commands.md). |
| `tabTypes` | object[] | no | Webview tab types the extension exposes. See [Tabs](tabs.md). |
| `panels` | object[] | no | Dockable/floating webview panels. See [Panels](panels.md). |
| `popovers` | object[] | no | Transient webview popovers anchored to a topbar/status-bar item. See [Popovers](popovers.md). |
| `topbarItems` | object[] | no | Icons attached to the tab strip. See [Topbar](topbar.md). |
| `statusBarItems` | object[] | no | Icons attached to the footer status bar. See [Status Bar](statusbar.md). |
| `settings` | object[] | no | Typed settings shown in the Settings sidebar. See [Settings](settings.md). |
| `aiProvider` | object | no | Optional notification source mapping. See [AI Provider Hooks](ai-provider.md). |

Extensions are enabled by default after loading. The Settings → Extensions toggle is persisted in `UserDefaults` under `muxy.ext.enabled.<extension-id>` and survives launches. A legacy `enabled` manifest field is no longer part of the schema; if present with no user override, it is migrated into that UserDefaults entry on first load and otherwise ignored.

## Icons

Topbar and status-bar items accept an `icon` field in one of two forms:

```json
{ "icon": { "symbol": "puzzlepiece.extension" } }
{ "icon": { "svg": "assets/badge.svg" } }
```

A bare string (`"icon": "puzzlepiece.extension"`) is shorthand for `{ "symbol": ... }`.

- **`symbol`** — any SF Symbol name. Tinted with the chrome's foreground color.
- **`svg`** — a path relative to the extension directory to a `.svg` file. The file must exist at load time, must not escape the extension directory, and must be at most 256 KiB. Rendered as a template image, so fills/strokes using `currentColor` (or a single solid color) pick up the chrome tint.

## Loader behaviour

`ExtensionStore` walks `~/.config/muxy/extensions/*/manifest.json` at app start. For each one it decodes the manifest, validates `name` against the allowed character set, verifies the `background` file resolves inside the extension directory (if declared), and refuses duplicate names. Failures appear in **Settings → Extensions → Load Errors**; the app does not retry until you click **Reload Extensions** or restart.

## Background script environment

A `background` script never speaks a wire protocol. Muxy handles the socket, identity token, and handshake; authors only use the `muxy` global it injects:

- `muxy.extensionID` — the extension's `name`.
- `muxy.events.subscribe(name, handler)` / `unsubscribe` — receive declared [events](events.md).
- `muxy.exec(argv[, options])` — run a shell command (needs `commands:exec`).
- `console.log` / `console.warn` / `console.error` — written to the extension log.

The richer state/mutation API (`tabs`, `panes`, `projects`, `worktrees`, etc.) is available only to tab/panel/popover pages via `window.muxy`, not to the background script.
