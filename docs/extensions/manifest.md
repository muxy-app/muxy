# Manifest

Every extension declares itself in a `manifest.json` next to its entrypoint.

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
  "aiProvider": null,
  "enabled": true
}
```

## Fields

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `name` | string | yes | Letters, digits, `-`, `_`, `.` only. Must match the directory name in practice. Used as the extension ID. |
| `version` | string | yes | Free-form. Shown in Settings → Extensions. |
| `description` | string | no | One-line description shown in Settings. |
| `entrypoint` | string | yes | Path (relative to manifest) to an executable file. Permission bit must be set. |
| `permissions` | string[] | no | See [Permissions](permissions.md). Verbs not in the list are rejected. Defaults to empty. |
| `events` | string[] | no | Events the extension is allowed to subscribe to. See [Events](events.md). Defaults to empty. |
| `commands` | object[] | no | Palette commands to register. See [Palette Commands](palette-commands.md). |
| `aiProvider` | object | no | Optional notification source mapping. See [AI Provider Hooks](ai-provider.md). |
| `enabled` | bool | no | Defaults to `true`. Toggling in Settings persists across launches at runtime. |

## Loader behaviour

`ExtensionStore` walks `~/.config/muxy/extensions/*/manifest.json` at app start. For each one it:

1. Decodes the manifest with JSON.
2. Validates `name` against the allowed character set.
3. Verifies `entrypoint` exists and is executable.
4. Refuses duplicates (same `name`); surfaces the second one as a load error in Settings.

Any failure is reported in **Settings → Extensions → Load Errors** with the directory name and reason. The app does not retry until you click **Reload Extensions** or restart Muxy.

## Subprocess environment

Each enabled extension is spawned with these environment variables:

| Variable | Value |
| --- | --- |
| `MUXY_SOCKET_PATH` | Absolute path to `muxy.sock` |
| `MUXY_EXTENSION_ID` | The extension's `name` from the manifest |

Both must be passed back when the extension connects — see [Events](events.md) for the handshake.
