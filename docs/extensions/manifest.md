# Manifest

The manifest is the `package.json` at the root of the extension. `name` and
`version` sit at the top level; everything Muxy-specific sits under `muxy`.

```json
{
  "name": "hello",
  "version": "0.1.0",
  "scripts": { "build": "vite build && cp package.json dist/" },
  "muxy": {
    "$schema": "https://raw.githubusercontent.com/muxy-app/muxy/main/docs/extensions/schema/manifest.schema.json",
    "description": "Says hello",
    "permissions": ["notifications:write"],
    "commands": [{ "id": "ping", "title": "Hello: Ping" }]
  }
}
```

The [JSON schema](schema/manifest.schema.json) gives editor completion. Muxy
ignores `$schema` and other keys it doesn't know.

## Loading

- Muxy reads `dist/package.json` when it exists, otherwise `package.json`.
- Every path in the manifest (`entry`, `background`, `script`, icons) is
  relative to `dist/` when the folder has one, otherwise to the folder itself.
  Paths can't leave that folder.
- An invalid manifest shows as a load error in **Settings → Extensions**.

## Top-level fields

| Field | Required | Notes |
| --- | --- | --- |
| `name` | yes | The extension ID. Use `a-z`, `0-9`, `.`, and `-`; pages don't load for other names. Must match the install folder name. |
| `version` | yes | Semver. Bump it for every published change. |
| `scripts.build` | to publish | Must leave `package.json` in `dist/`. |

## `muxy` fields

All optional.

| Field | Type | See |
| --- | --- | --- |
| `description` | string | One line, shown in Settings. |
| `permissions` | string[] | [Permissions](permissions.md) |
| `events` | string[] | Workspace events it may receive. [Events](events.md) |
| `background` | string | Path to a script that runs while the extension is enabled. [Events](events.md#background-scripts) |
| `commands` | object[] | [Commands](commands.md) |
| `tabTypes` | object[] | [Tabs](tabs.md) |
| `fileOpeners` | object[] | [Tabs](tabs.md#file-openers) |
| `panels` | object[] | [Panels](panels.md) |
| `popovers` | object[] | [Panels](panels.md#popovers) |
| `sidebar` | object | [Panels](panels.md#sidebar) |
| `topbarItems` | object[] | [Bar items](bar-items.md) |
| `statusBarItems` | object[] | [Bar items](bar-items.md) |
| `localizations` | object[] | [Localizations](localizations.md) |
| `settings` | object[] | [Settings](#settings) below |
| `marketplace` | object | Listing details. [Contributing](contributing.md) |

`homeViews` and `remoteMethods` are accepted for 1.x compatibility but do
nothing yet.

IDs must be unique within their list, and every reference (a command's panel,
an item's command) must exist.

## Icons

Panels, the sidebar, and bar items take an `icon`:

```json
{ "icon": "sparkles" }
{ "icon": { "symbol": "sparkles" } }
{ "icon": { "svg": "assets/icon.svg" } }
```

- `symbol` is an [SF Symbol](https://developer.apple.com/sf-symbols/) name.
- `svg` is a file of up to 256 KiB. It is tinted like the app's own icons, so
  use `currentColor` or one solid color.

## Settings

Declared settings appear on the extension's page in **Settings → Extensions**.

```json
"settings": [
  { "key": "greeting", "title": "Greeting", "type": "string", "defaultValue": "Hello" },
  { "key": "loud", "title": "Shout", "type": "bool" }
]
```

`type` is `string`, `bool`, or `number`. Extensions can't read these values at
runtime yet; use [storage](api.md#storage) for values your code needs.
