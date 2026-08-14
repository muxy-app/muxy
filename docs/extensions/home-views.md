# Extension Home Views

A home view declares a global, full-window extension page that is independent of project tabs and their restored layouts. Use one for an overview or launch destination that spans projects and worktrees. Use a [tab](tabs.md) instead when the page belongs to the active project workspace and should participate in tab restoration.

> **Current availability:** Muxy currently decodes and validates the `homeViews` manifest contract. App navigation, launch-destination selection, fallback behavior, and visible presentation are integrated separately. Declaring a home view alone does not yet add a visible destination.

## Declaring a home view

Add `homeViews` to the `muxy` object in `package.json`:

```json
{
  "name": "workspace-overview",
  "version": "0.1.0",
  "scripts": {
    "build": "vite build && node scripts/copy-manifest.mjs"
  },
  "muxy": {
    "homeViews": [
      {
        "id": "overview",
        "title": "Workspace Overview",
        "icon": "rectangle.3.group",
        "entry": "overview/index.html",
        "defaultData": {
          "showIdle": false
        }
      }
    ]
  }
}
```

### Fields

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `id` | string | yes | Non-empty identifier that is unique within the extension. Keep it stable across releases. |
| `title` | string | yes | Non-empty user-facing title. |
| `icon` | string \| object | no | Non-empty SF Symbol name, `{ "symbol": "…" }`, or `{ "svg": "assets/icon.svg" }`. |
| `entry` | string | yes | Non-empty HTML path relative to the build output. It must exist and resolve inside the extension directory. |
| `defaultData` | JSON value | no | Initial data retained for the home-view webview. |

An extension may declare multiple home views, but every `id` must be unique within that extension.

## Resources and validation

Muxy resolves `entry` and SVG icon paths against the installed build output, normally `dist/`. Paths may use any internal directory layout, but they cannot escape the extension directory through `..` components or symbolic links.

SVG icons must:

- Use a relative path ending in `.svg`.
- Exist when the extension is loaded.
- Resolve inside the extension directory.
- Be no larger than 256 KiB.

A home-view declaration does not grant permissions or allow an extension to select itself as the default launch destination. Any APIs used by its page remain subject to the permissions declared in the manifest. Default selection and fallback are app-owned behavior rather than manifest fields.
