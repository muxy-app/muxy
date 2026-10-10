# API Reference

Muxy injects a `muxy` object into extension code. On pages, most calls return
a Promise. In scripts and background scripts, most calls are synchronous.
Exceptions are noted where they apply, such as `execAsync`, which returns a
handle whose `result` you await.

```js
const tabs = await muxy.tabs.list();   // page
const tabs = muxy.tabs.list();         // script or background
```

Most calls need a [permission](permissions.md). Errors throw (or reject) with
a message.

## Availability

| Namespace | Pages | Scripts | Background |
| --- | :-: | :-: | :-: |
| `notifications`, `dialog`, `storage`, `shortcuts`, `topbar`, `statusbar`, `agents`, `gh`, `git`, `exec`, `modal` | ✓ | ✓ | ✓ |
| `tabs.open` | ✓ | ✓ | ✓ |
| other `tabs`, `panes`, `projects`, `workspaces`, `worktrees`, `files`, `toast`, `execAsync` | ✓ | ✓ | |
| `events` | ✓ | | ✓ |
| `panels`, `popover`, `http`, `lifecycle`, `data`, `theme` | ✓ | | |

Scripts and background scripts also have `setTimeout`, `setInterval`, and
their `clear` functions.

## Workspace

| Call | Returns |
| --- | --- |
| `projects.list()` | `[{ id, name, path, isActive, sortOrder, iconColor, icon, logo, worktreesEnabled }]` |
| `projects.switchTo(id)`, `add(path)`, `create(path, { name, workspace, createIfMissing })` | |
| `projects.rename(id, name)`, `setColor(id, color)`, `setIcon(id, icon)`, `reorder(ids)` | |
| `projects.attach(id, workspace)`, `detach(id)`, `delete(id)` | |
| `workspaces.list()` | `[{ id, name, projectCount, isActive }]` |
| `workspaces.create(name)`, `switchTo(id)`, `rename(id, name)`, `delete(id)` | |
| `worktrees.list(project?)` | `[{ id, name, path, branch, isActive }]` |
| `worktrees.switchTo(id, project?)`, `refresh(project?)` | |
| `tabs.list()` | `[{ index, id, kind, title, isActive }]` for the current project |
| `tabs.switchTo(indexOrIdOrTitle)`, `new()`, `next()`, `previous()` | |
| `tabs.open(request)` | The new tab's ID. See [Tabs](tabs.md#opening-tabs) |
| `panes.list()` | `[{ id, title, workingDirectory, isFocused }]` across projects |
| `panes.send(id, text)`, `sendKeys(id, key)` | |
| `panes.readScreen(id, lines = 50)` | The last rows of the screen as text, up to 500 |
| `panes.close(id)`, `rename(id, title)` | |
| `agents.list()` | Agent status per worktree, like the `agent.status` event |

`send` and `sendKeys` only reach panes that are on screen. For a hidden pane,
`readScreen` returns the text it last showed.

## Running programs

```js
const result = await muxy.exec(["git", "status", "--short"], { cwd: "src" });
const result = await muxy.exec({ shell: "ls | wc -l" });
// { stdout, stderr, exitCode, timedOut, truncated }
```

- Runs on the project's server, in the project folder, with your login `PATH`
  and `MUXY_EXTENSION_ID` set.
- Options: `cwd`, `env`, `stdin`, `timeoutMs` (default 30 s, up to 300 s).
- Output is capped at 4 MiB per stream.
- `execAsync(...)` returns `{ id, result, cancel }` right away, so a page or
  script can cancel a long run.

## Storage

A small key-value store per extension, kept after uninstall.

```js
await muxy.storage.set("count", 3);
await muxy.storage.get("count");   // 3, or null
await muxy.storage.delete("count");
await muxy.storage.keys();         // sorted keys
```

Values are JSON, up to 1 MiB each and 5 MiB in total.

## Web requests

`http.fetch(url, { method, headers, body, timeoutMs })` resolves to
`{ status, headers, body, truncated }`. It runs in the app, asks the user per
host, and can't reach the local machine or private networks. Response bodies
are capped at 10 MiB and timeouts at 120 s. Pages only.

## Notifications

- `notifications.notify({ title, body })` posts a notification and the
  `notification.posted` event.
- `toast({ title, body })` does the same from pages and scripts.

## Shortcuts

`shortcuts.register({ id, combo })` binds a key to one of the extension's
commands while it runs. `combo` looks like `cmd+shift+e` or `opt+left` and
needs `cmd`, `ctrl`, or `opt`. `unregister(id)` and `list()` manage them.

## GitHub

`gh.user()` returns the signed-in GitHub user from the `gh` CLI on the desktop
computer, even for remote projects. It is cached for five minutes.

## Page only

| Member | What it is |
| --- | --- |
| `extensionID` | The extension's name (everywhere) |
| `tabInstanceID`, `panelID` | This view's ID |
| `data`, `onDataChange(fn)` | The view's data. See [Tabs](tabs.md#page-data) |
| `theme`, `onThemeChange(fn)` | Theme colors. See [Tabs](tabs.md#theme) |
| `focused`, `onFocus(fn)` | Whether the view has focus |
| `lifecycle.onBeforeClose(fn)` | Called before the view closes. Return `true`, or a Promise of it, to keep it open. A page that doesn't respond is closed after 5 s |
| `lifecycle.close()` | Closes this view |

More: [Events](events.md), [Tabs](tabs.md), [Panels](panels.md),
[Bar items](bar-items.md), [Dialogs](dialogs.md), [Files](files.md),
[Git](git.md).
