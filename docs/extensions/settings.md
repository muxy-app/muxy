# Settings

Extensions can declare typed settings that appear in their own row in the Settings sidebar. The values are stored per-extension and can be read or written from the extension subprocess.

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
| `key` | string | yes | Unique within the extension. Persisted as `muxy.ext.<extension-id>.<key>` in app defaults. |
| `title` | string | yes | Row label in the Settings UI. |
| `description` | string | no | Sub-text shown below the row. |
| `type` | string | yes | `string`, `bool`, or `number`. Controls the renderer and JSON type. |
| `defaultValue` | any | no | JSON value used when the user has not set the key. Type should match `type`. |

## UI

The sidebar adds one row per enabled extension that declares at least one setting, immediately below the built-in **Extensions** row. The row icon is the standard puzzle-piece glyph and the title is the extension's display name. The detail pane lists each setting as a labeled control:

- `bool` → toggle switch
- `string` → text input
- `number` → text input (parses as `Double`; an empty field resets to default)

## Runtime API

The extension reads and writes its own settings over the existing notification socket. The verbs are scoped to the calling extension — there is no permission to declare. They use JSON-encoded values for round-trippable types.

```
identify|<extension-id>
extension.settings.get|<key>
extension.settings.set|<key>|<json-value>
```

### Get

| Response | Meaning |
| --- | --- |
| `ok\t<json>` | Current effective value. JSON `null` is returned when no override exists and there is no `defaultValue`. |
| `error:setting '<key>' not declared in manifest` | Add the key under `settings` in the manifest. |
| `error:identify required` | Connection has not called `identify` yet. |

### Set

The third pipe-separated argument is a single JSON value — `true`, `42`, `"hello"`, etc. Pipes inside JSON strings are passed through verbatim (the verb concatenates the remainder of the message).

| Response | Meaning |
| --- | --- |
| `ok` | Value stored. |
| `error:invalid json value: …` | The payload could not be JSON-decoded. |
| `error:setting '<key>' not declared in manifest` | Add the key under `settings` in the manifest. |

## Storage

Values live under `UserDefaults.standard` with keys of the form `muxy.ext.<extension-id>.<key>`. They survive app restarts but are not synced across machines. Disabling an extension does not clear its settings; uninstalling does not either (settings persist by design so a re-installed extension keeps its configuration).
