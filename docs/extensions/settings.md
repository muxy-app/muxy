# Settings

An extension can declare typed settings that get their own row in the Settings sidebar. Values are stored per-extension and persist across restarts.

```json
{
  "settings": [
    {
      "key": "endpoint",
      "title": "API Endpoint",
      "description": "Base URL for the build server.",
      "type": "string",
      "defaultValue": "https://builds.example.com"
    },
    {
      "key": "notify",
      "title": "Notify on Failure",
      "type": "bool",
      "defaultValue": true
    }
  ]
}
```

## Entry fields

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `key` | string | yes | Unique within the extension. Persisted as `muxy.ext.<extension-id>.<key>`. |
| `title` | string | yes | Row label in the Settings UI. |
| `description` | string | no | Sub-text shown below the row. |
| `type` | string | yes | `string`, `bool`, or `number`. Controls the renderer and JSON type. |
| `defaultValue` | any | no | JSON value used when the user has not set the key. Type should match `type`. |

## UI

Each enabled extension with at least one setting gets a row below the built-in **Extensions** row. The detail pane renders each setting as a labeled control:

- `bool` → toggle switch
- `string` → text input
- `number` → text input (empty field resets to default)

## Runtime API

Settings can be read and written at runtime over the **socket** — the `extension.settings.get` / `extension.settings.set` verbs, used by the `muxy` CLI and advanced integrations. They are **not** currently exposed as methods on the background `muxy` global, so a `background.js` script cannot read or write settings directly today.

Muxy scopes these verbs to the calling extension via its identity; callers do not perform the handshake themselves. The socket contract:

### Get — `extension.settings.get|<key>`

| Response | Meaning |
| --- | --- |
| `ok` (no payload) | No override and no `defaultValue` — the setting is unset. |
| `ok\t<json>` | Current effective value, JSON-encoded. A literal `null` means the stored value is JSON null (distinct from "unset"). |
| `error:setting '<key>' not declared in manifest` | Add the key under `settings`. |
| `error:identify required` | The connection has not been identified yet. |

### Set — `extension.settings.set|<key>|<json-value>`

The value is a single JSON value (`true`, `42`, `"hello"`, …). Total payload must be at most 64 KiB.

| Response | Meaning |
| --- | --- |
| `ok` | Value stored. |
| `error:invalid json value: …` | The payload could not be JSON-decoded. |
| `error:setting '<key>' not declared in manifest` | Add the key under `settings`. |
| `error:value exceeds 65536-byte limit` | Payload too large. |

## Storage

Values live in `UserDefaults` under `muxy.ext.<extension-id>.<key>`. They persist across app restarts and survive disabling or uninstalling the extension, so a re-installed extension keeps its configuration. They are not synced across machines.
