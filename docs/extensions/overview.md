# Extensions Overview

> **Status:** under active development (**DEV** in Settings). Manifest format, permissions, and event payloads may change without notice.

Extensions are npm + [Vite](https://vitejs.dev) projects that Muxy loads on launch. Authors build them with any framework; `npm run build` emits a `dist/` directory, and that build output is what is published and installed. Extensions react to workspace events, coordinate webviews with background scripts, register palette commands, add UI (tabs, panels, popovers, topbar/status-bar items), and — with permission — drive the same verbs the `muxy` CLI exposes.

## Architecture

Muxy's main process (`ExtensionStore`) scans the extensions directory, loads the enabled ones, and gives each two surfaces:

- **Declared UI** — panels, tabs, popovers, topbar/status-bar items render in-process as WKWebViews. Their pages talk to Muxy through the injected `window.muxy` bridge, which exposes the full API (`tabs`, `panes`, `projects`, `worktrees`, `events`, `exec`, `toast`, `panels`, `popover`). No subprocess.
- **Background script** — if the manifest declares a `muxy.background` script, Muxy runs it in a small bundled host process (`MuxyExtensionHost`). This is where durable event listeners (`muxy.events.subscribe`), webview coordination over `extension.*` events, and `muxy.exec` live. `setTimeout`/`setInterval` (and their `clear*` counterparts) are available for polling or scheduled work. Most extensions don't need one.

Workspace events originate in the main process (`ExtensionEventEmitter` diffs workspace state) and are delivered to the host. Extension-local `extension.*` events are scoped to one extension and pass between that extension's open webviews and background script. `muxy.exec` is gated by a permission check and a runtime consent prompt before it runs.

## Pages

| Page | What's in it |
| --- | --- |
| [Manifest](manifest.md) | `package.json` `muxy` fields and examples |
| [Permissions](permissions.md) | Permission grants and runtime consent |
| [Events](events.md) | Subscribable events and payloads |
| [Palette Commands](palette-commands.md) | Commands that appear in the command palette |

## Where extensions live

Each installed extension is the `dist/` produced by `npm run build`, with its `package.json` at the root:

```
~/.config/muxy/extensions/
  <name>/
    package.json
    background.js     # optional; pushed events / extension.* bus / background exec
    …                 # the rest of the build output
```

`ExtensionStore` scans this directory at app start, reads each `package.json` (top-level `name`/`version` plus the `muxy` object), validates it, and runs the `background.js` of each enabled extension that declares one. Settings → Extensions lists every loaded extension with a toggle, its permissions, and recent log output.

## Security model

- **Manifest-declared permissions.** Every state-changing verb requires a matching `muxy.permissions` entry. See [Permissions](permissions.md).
- **Subscription allowlist.** An extension may subscribe only to events declared in its `muxy.events` array, to its own `command.<id>` events, or to same-extension `extension.*` events.
- **Runtime consent.** Verbs that run code or read terminal contents prompt the user even when the permission is granted.
- **Process isolation.** The background process runs out-of-process; a crash is surfaced in Settings and can't take down Muxy. `console.*` output is captured to an in-app rolling log.
- **Loaded-from-disk only.** Muxy only runs the `background.js` of an extension it actually loaded.
