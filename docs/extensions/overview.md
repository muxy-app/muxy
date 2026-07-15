# Extensions Overview

> **New here?** Start with [Get started](get-started.md) to build and run an extension first.

Extensions are npm + Vite projects. Muxy installs the built `dist/` directory and reads its `package.json` to register UI, commands, permissions, and optional background behavior.

## Runtime model

| Surface | Runtime | Use it for |
| --- | --- | --- |
| Tabs, panels, popovers, sidebars, and webview modals | `WKWebView` in the main app | Visible UI with the full `window.muxy` bridge |
| `runScript` commands | JavaScriptCore in the main app | One-shot work launched from a command or shortcut |
| `background.js` | `MuxyExtensionHost` subprocess | Durable events, shared webview coordination, and headless commands |

Declared UI talks directly to the main process through `window.muxy`; it does not use the extension socket. Background scripts connect through Muxy's authenticated Unix-socket bridge. Workspace events and same-extension `extension.*` messages are brokered by the main process.

Most extensions need only declared UI or `runScript` commands. Add a background script only when work must continue without an open webview.

## Installation

Each installed extension is the output of `npm run build`:

```
~/.config/muxy/extensions/<name>/
  package.json
  …
```

The build must copy `package.json` and any referenced background script into `dist/`. Muxy validates the manifest and referenced files before loading the extension. See [Manifest](manifest.md) for the complete contract.

## Security

- Manifest permissions limit access to gated API calls.
- Sensitive operations also require runtime consent.
- Event subscriptions are limited to declared workspace events, extension commands, and same-extension messages.
- Background scripts run out of process, and their logs are captured separately.
- Muxy runs background code only from an extension it loaded and identified.

See [Permissions](permissions.md) for permission and consent behavior.

## Next steps

- [Get started](get-started.md) — scaffold and run an extension.
- [Manifest](manifest.md) — declare surfaces and capabilities.
- [Events](events.md) — react to workspace changes.
- [Contributing](contributing.md) — validate and publish an extension.
