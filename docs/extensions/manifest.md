# Manifest

Every extension declares itself in a `manifest.json` at the root of its directory.

```json
{
  "name": "hello",
  "version": "0.1.0",
  "description": "Demo extension that subscribes to events and exposes a palette command",
  "entrypoint": "run.sh",
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
| `name` | string | yes | Letters, digits, `-`, `_`, `.` only. Must match the directory name in practice. Used as the extension ID. |
| `version` | string | yes | Free-form. Shown in Settings → Extensions. |
| `description` | string | no | One-line description shown in Settings. |
| `entrypoint` | string | no | Path (relative to manifest) to an executable file. Permission bit must be set. Provide it only when the extension needs to receive pushed [events](events.md); Muxy launches it as a long-lived subprocess. Command, topbar, status bar, tab, and `runScript` extensions need none. |
| `permissions` | string[] | no | See [Permissions](permissions.md). Verbs not in the list are rejected. Defaults to empty. |
| `events` | string[] | no | Events the extension is allowed to subscribe to. See [Events](events.md). Defaults to empty. |
| `commands` | object[] | no | Palette commands to register. See [Palette Commands](palette-commands.md). |
| `tabTypes` | object[] | no | Webview tab types the extension exposes. See [Tabs](tabs.md). |
| `panels` | object[] | no | Dockable/floating webview panels the extension exposes. See [Panels](panels.md). |
| `topbarItems` | object[] | no | Icons to attach to the tab strip. See [Topbar](topbar.md). |
| `statusBarItems` | object[] | no | Icons to attach to the footer status bar. See [Status Bar](statusbar.md). |
| `settings` | object[] | no | Typed settings shown in the Settings sidebar. See [Settings](settings.md). |
| `aiProvider` | object | no | Optional notification source mapping. See [AI Provider Hooks](ai-provider.md). |

Extensions are enabled by default after loading. Users toggle them in **Settings → Extensions**; that toggle is persisted in `UserDefaults` under `muxy.ext.enabled.<extension-id>` and survives across launches.

A legacy `enabled` field on the manifest is no longer part of the schema. If present and no user override exists yet, it is migrated into the UserDefaults entry above on first load and otherwise ignored.

## Icons

Topbar and status bar items accept an `icon` field in one of two forms:

```json
{ "icon": { "symbol": "puzzlepiece.extension" } }
{ "icon": { "svg": "assets/badge.svg" } }
```

A bare string (`"icon": "puzzlepiece.extension"`) is accepted as shorthand for `{ "symbol": ... }`.

- **`symbol`** — any SF Symbol name. Tinted with the chrome's foreground color (topbar items also pick up a hover color).
- **`svg`** — a path **relative to the extension directory** to a file with a `.svg` extension. The file must exist at load time, must not escape the extension directory, and must be at most 256 KiB. Rendered as a *template* image, so SVG fills/strokes that use `currentColor` (or a single solid color) pick up the chrome tint.

## Loader behaviour

`ExtensionStore` walks `~/.config/muxy/extensions/*/manifest.json` at app start. For each one it:

1. Decodes the manifest with JSON.
2. Validates `name` against the allowed character set.
3. If `entrypoint` is present, verifies it exists and is executable.
4. Refuses duplicates (same `name`); surfaces the second one as a load error in Settings.

Any failure is reported in **Settings → Extensions → Load Errors** with the directory name and reason. The app does not retry until you click **Reload Extensions** or restart Muxy.

## Subprocess environment

Each enabled extension that declares an `entrypoint` is spawned with these environment variables:

| Variable | Value |
| --- | --- |
| `MUXY_SOCKET_PATH` | Absolute path to `muxy.sock` |
| `MUXY_EXTENSION_ID` | The extension's `name` from the manifest |
| `MUXY_EXTENSION_TOKEN` | Random per-launch token. Required as the third argument of `identify`. |

All three must be passed back when the extension connects — see [Events](events.md) for the handshake.
