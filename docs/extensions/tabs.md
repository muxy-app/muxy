# Tabs

A tab type is a web page that opens as a tab, next to terminal tabs. It can be
split, moved, and pinned like any tab, and is restored on launch.

```json
"tabTypes": [
  { "id": "viewer", "title": "Viewer", "entry": "viewer/index.html", "defaultData": { "mode": "list" } }
]
```

| Field | Notes |
| --- | --- |
| `id` | Unique within the extension. |
| `title` | The tab's default title. |
| `entry` | HTML page in the extension folder. |
| `defaultData` | Data used when the tab is opened without any. |

Open a tab type from a [command](commands.md) with the `openTab` action, or
from code.

## Opening tabs

`tabs.open` needs `tabs:write` and returns the new tab's ID.

```js
// a page from this or another extension
await muxy.tabs.open({
  kind: "extensionWebView",
  extension: { id: "hello", tabType: "viewer", data: { mode: "grid" }, singleton: true },
});

// a terminal in the current project, optionally in a subfolder or running a command
await muxy.tabs.open({ kind: "terminal", directory: "src", command: "npm test" });

// a browser tab; drive it with muxy.browser
await muxy.tabs.open({ kind: "browser", url: "localhost:3000" });
```

- `singleton: true` focuses an existing tab of that type instead of opening
  another.
- Running a command, or opening another extension's tab, asks the user first.
- A terminal's `directory` must be a folder inside the current project, and
  works for local projects only.

## Tab groups

Tab groups show several of a project's tabs side by side. `tabs.split` does
what dragging a tab to an edge does: it moves the tab into a new group beside
the group showing another tab, and focuses it. Both tabs must belong to the
current project.

```js
const left = await muxy.tabs.open({ kind: "terminal" });
const right = await muxy.tabs.open({ kind: "terminal", directory: "server" });
await muxy.tabs.split(right, left, "right");
// opened next: joins the focused group, on the right
await muxy.tabs.open({ kind: "terminal", directory: "web" });
```

`edge` is `left`, `right` (default), `up`, or `down`. A group's only tab can't
split off beside its own group.

## Page data

Each tab has its own `data`, saved with the tab.

```js
render(muxy.data);
muxy.onDataChange(render);
muxy.tabs.setTitle("Report");
muxy.tabs.setIcon({ symbol: "doc.text" });   // or { svg: "..." }, or null
```

## Theme

Pages get the app's colors as CSS variables, updated live:

```css
body {
  background: var(--muxy-background);
  color: var(--muxy-foreground);
  border-color: var(--muxy-border);
}
```

Variables: `--muxy-background`, `--muxy-foreground`, `--muxy-foreground-muted`,
`--muxy-surface`, `--muxy-surface-solid`, `--muxy-border`, `--muxy-hover`,
`--muxy-accent`, `--muxy-accent-foreground`, `--muxy-accent-soft`,
`--muxy-diff-add`, `--muxy-diff-remove`, `--muxy-diff-hunk`, and
`--muxy-topbar-height`. The same values are in `muxy.theme`, with
`colorScheme` set to `light` or `dark`. `muxy.onThemeChange(fn)` reports
changes.

Muxy draws `--muxy-background` behind every page, unless a panel is
`transparent`. It shows while the page loads and wherever the page leaves its
background unset, so a page never flashes white as it opens.

## File openers

A file opener lets an extension open files that users `Cmd`-click in a
terminal, in one of its tab types.

```json
"fileOpeners": [
  { "id": "markdown", "title": "Markdown preview", "tabType": "viewer", "patterns": ["*.md"] }
]
```

| Field | Notes |
| --- | --- |
| `tabType` | One of the extension's tab types. |
| `patterns` | Default file types, using case-insensitive `*`/`?` globs. Patterns without `/` match the filename at any depth; patterns with `/` match the project-relative path. Omitted or empty means no automatic matches. Use `["*"]` explicitly to support every file. |
| `fileTypes` | Optional automatic registrations, using the same globs. When present, overrides `patterns` in new Muxy versions, including an empty list. Older Muxy versions ignore it and use `patterns`. |
| `singleton` | Reuse one tab. Default `true`. |

Enabled extensions register these patterns automatically. One matching opener
opens the file directly. If several match, Muxy shows a searchable chooser.
The user can open once or turn on **Always use for this type** before selecting
an opener. With no matching extension, local files are revealed in Finder (or
the system file manager on Linux). Directories and files outside the project
also use the file manager. Remote files retain their remote-file actions.

**Settings → General → Files** lists registrations and lets users add custom
patterns, choose or remove defaults, reorder rules, select **Always ask** for a
pattern, or enable **Always show the file opener**. A custom rule can assign a
file type to an extension even when it is absent from the manifest's patterns.
The first matching rule with an available opener wins. Unavailable extension
rules are ignored until the extension is enabled again. The topbar project-app
picker is independent of terminal file links.

The tab receives `data` like
`{ "filePath": "docs/README.md", "line": 12, "column": 3, "source": "terminal" }`,
with `filePath` relative to the clicked file's project. Local paths are resolved
before dispatch, so symlinks must resolve inside that project. The opener
receives the resolved relative path.

### Supporting older Muxy versions

Keep the existing opener ID, tab type, `patterns`, and data contract. Add
`fileTypes` to advertise precise automatic registrations without changing how
older versions handle an explicitly selected opener:

```json
"fileOpeners": [
  {
    "id": "editor",
    "tabType": "code-editor",
    "patterns": ["*"],
    "fileTypes": ["*.md", "*.txt", "*.rs", "*.png", "*.pdf", "Dockerfile"],
    "singleton": false
  }
]
```

New Muxy versions use `fileTypes`; older versions continue using `patterns` and
their global opener preference. No new JavaScript API is required. Publish the
updated manifest schema before releasing an extension that uses the new field.
