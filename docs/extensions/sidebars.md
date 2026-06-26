# Extension Sidebars

A sidebar is a full-height webview that **replaces Muxy's built-in left sidebar** when the user selects it. An extension declares at most one; the user picks the active sidebar in **Settings → Sidebar**. Each sidebar is a `WKWebView` with the injected [`window.muxy`](tabs.md#windowmuxy) bridge, just like a [tab](tabs.md) or [panel](panels.md).

When active, the extension webview fills the **entire** sidebar region — both the built-in project list and the bottom footer (extensions button, sidebar toggle, theme picker, notifications) are replaced. The extension owns its own navigation and controls.

## Declaring a sidebar

```json
{
  "name": "workspace-nav",
  "version": "0.1.0",
  "muxy": {
    "sidebar": {
      "id": "main",
      "title": "My Sidebar",
      "icon": "sparkles",
      "entry": "sidebar/index.html",
      "defaultData": {}
    }
  }
}
```

`sidebar` is a single object, not an array — one sidebar per extension.

### Fields

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `id` | string | yes | Stable per extension. |
| `entry` | string | yes | HTML path relative to the build output — any layout works (e.g. `sidebar/index.html`); not a fixed folder. Must resolve inside the extension directory (no `..` traversal). |
| `title` | string | no | Shown next to the extension in the **Active Sidebar** dropdown. |
| `icon` | string \| object | no | SF Symbol name, or `{ "svg": "assets/icon.svg" }`. Shown in the dropdown. |
| `defaultData` | object | no | JSON exposed to the page as `window.muxy.data`. |

## Activation

The user chooses the active sidebar from an **Active Sidebar** dropdown in **Settings → Sidebar** — **Built-in** or any enabled extension that declares a sidebar. The dropdown only appears when at least one enabled extension declares a sidebar.

The toggle-sidebar shortcut (`⌘B`) and the resize handle act on the whole sidebar region regardless of which sidebar is active. A sidebar cannot close or deselect itself — selection is controlled by the user in settings, so there is no `closeSelf` for this surface.

## Theming

The sidebar paints `--muxy-background` on the body, like [tabs](tabs.md) and [panels](panels.md). Use the injected `--muxy-*` theme variables for every color so it tracks the live theme and inverts for light/dark automatically.

## window.muxy

A sidebar page gets the same [`window.muxy`](tabs.md#windowmuxy) API as panels and tabs — theme (`muxy.theme`, `muxy.onThemeChange`), data (`muxy.data`, `muxy.onDataChange`), focus (`muxy.onFocus`, `muxy.focused`), and the workspace surfaces (`projects`, `git`, `files`, `events`, …) gated by their permissions. Because the sidebar replaces the built-in project list, `muxy.projects` (`list` / `switchTo`) is the usual way to render and switch projects. Each `list()` entry carries `id`, `name`, `path`, `isActive`, `sortOrder`, `worktreesEnabled`, and the nullable `iconColor` / `icon` / `logo` — enough to render the list (color, icon, order) without reading `projects.json`. With `projects:write` you can also mutate the live project list — `add(path)` (which also activates the added project), `rename(identifier, name)`, `setColor`, `setIcon`, `setLogo`, and `reorder(identifiers)` with all local non-home project ids — and subscribe to the `projects.changed` event to refetch after any change (native or extension-driven). `muxy.projects.delete(identifier)` deletes a project — it requires the `projects:delete` permission and prompts the user for confirmation on each call.

## Reacting to workspace changes

A sidebar is created once and persists across the session — it does **not** reload when the active project, worktree, or tab changes. `muxy.projects.list()` reads live state at call time, so to keep the active highlight correct you must re-fetch on the relevant [event](events.md) instead of relying on the initial load:

```js
muxy.events.subscribe('project.switched', async () => {
  render(await muxy.projects.list());
});
```

Workspace events (`project.switched`, `worktree.switched`, `tab.focused`, …) must be **declared in your manifest `events` array** before you can subscribe — subscribing to an undeclared event is rejected and your handler silently never fires. See [Events](events.md) for the full list.

```json
"muxy": {
  "events": ["project.switched", "worktree.switched", "tab.focused"]
}
```

## Minimal example

```
workspace-nav/
  package.json
  vite.config.js
  src/sidebar/index.html
```

```json
{
  "name": "workspace-nav",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": { "dev": "vite", "build": "vite build" },
  "devDependencies": { "vite": "^5.0.0" },
  "muxy": {
    "permissions": ["projects:read", "projects:write"],
    "events": ["project.switched"],
    "sidebar": {
      "id": "main",
      "title": "Workspace",
      "icon": "sidebar.left",
      "entry": "sidebar/index.html"
    }
  }
}
```

```html
<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <style>
      body {
        margin: 0;
        height: 100vh;
        font: 12px -apple-system, system-ui, sans-serif;
        background: var(--muxy-background);
        color: var(--muxy-foreground);
      }
      .row {
        padding: 8px 10px;
        cursor: pointer;
      }
      .row:hover {
        background: var(--muxy-hover);
      }
    </style>
  </head>
  <body>
    <div id="projects"></div>
    <script type="module">
      const list = document.getElementById('projects');
      const render = (projects) => {
        list.replaceChildren(
          ...projects.map((project) => {
            const row = document.createElement('div');
            row.className = 'row';
            row.textContent = project.isActive ? `• ${project.name}` : project.name;
            row.onclick = () => muxy.projects.switchTo(project.id);
            return row;
          }),
        );
      };
      const refresh = async () => render(await muxy.projects.list());
      muxy.events.subscribe('project.switched', refresh);
      refresh();
    </script>
  </body>
</html>
```
