# Extensions Overview

> **Status:** under active development (**DEV** in Settings). Manifest format, permissions, and event payloads may change without notice.

Extensions are user-installed directories that Muxy loads on launch. They react to workspace events, register palette commands, add UI (tabs, panels, popovers, topbar/status-bar items), and — with permission — drive the same verbs the `muxy` CLI exposes.

## Architecture

Muxy's main process (`ExtensionStore`) scans the extensions directory, loads the enabled ones, and gives each two surfaces:

- **Declared UI** — panels, tabs, popovers, topbar/status-bar items render in-process as WKWebViews. Their pages talk to Muxy through the injected `window.muxy` bridge, which exposes the full API (`tabs`, `panes`, `projects`, `worktrees`, `events`, `exec`, `toast`, `panels`, `popover`). No subprocess.
- **Background script** — if the manifest declares a `background` script, Muxy runs it in a small bundled host process (`MuxyExtensionHost`). This is where event listeners (`muxy.events.subscribe`) and `muxy.exec` live. Most extensions don't need one.

Events originate in the main process (`ExtensionEventEmitter` diffs workspace state) and are delivered to the host. `muxy.exec` is gated by a permission check and a runtime consent prompt before it runs.

## Pages

| Page | What's in it |
| --- | --- |
| [Manifest](manifest.md) | `manifest.json` fields and examples |
| [Permissions](permissions.md) | Permission grants and runtime consent |
| [Events](events.md) | Subscribable events and payloads |
| [Palette Commands](palette-commands.md) | Commands that appear in the command palette |
| [AI Provider Hooks](ai-provider.md) | Route third-party notifications to a custom source |

## Where extensions live

```
~/.config/muxy/extensions/
  <name>/
    manifest.json
    background.js     # optional; only for pushed events / background exec
```

`ExtensionStore` scans this directory at app start, validates each manifest, and runs the `background.js` of each enabled extension that declares one. Settings → Extensions lists every loaded extension with a toggle, its permissions, and recent log output.

## Security model

- **Manifest-declared permissions.** Every state-changing verb requires a matching `permissions` entry. See [Permissions](permissions.md).
- **Subscription allowlist.** An extension may subscribe only to events declared in its manifest `events` array, or to its own `command.<id>` events.
- **Runtime consent.** Verbs that run code or read terminal contents prompt the user even when the permission is granted.
- **Process isolation.** The background process runs out-of-process; a crash is surfaced in Settings and can't take down Muxy. `console.*` output is captured to an in-app rolling log.
- **Loaded-from-disk only.** Muxy only runs the `background.js` of an extension it actually loaded.
