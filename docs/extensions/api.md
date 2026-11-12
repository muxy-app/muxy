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
| `notifications`, `dialog`, `storage`, `shortcuts`, `topbar`, `statusbar`, `agents`, `gh`, `git`, `exec`, `modal`, `panels` | ✓ | ✓ | ✓ |
| `app`, `ui`, `commands`, `composer`, `layouts`, `preferences`, `settings`, `clipboard` | ✓ | ✓ | ✓ |
| `tabs.open` | ✓ | ✓ | ✓ |
| other `tabs`, `panes`, `projects`, `workspaces`, `worktrees`, `files`, `toast`, `execAsync` | ✓ | ✓ | |
| `events` | ✓ | | ✓ |
| `tabs.setTitle`, `tabs.setIcon`, `panel`, `popover`, `http`, `lifecycle`, `data`, `theme` | ✓ | | |

Scripts and background scripts also have `setTimeout`, `setInterval`, and
their `clear` functions.

## Workspace

| Call | Returns |
| --- | --- |
| `projects.list()` | `[{ id, name, path, isActive, sortOrder, iconColor, icon, logo, worktreesEnabled }]` |
| `projects.switchTo(id)`, `add(path)`, `create(path, { name, workspace, createIfMissing })` | |
| `projects.rename(id, name)`, `setColor(id, color)`, `setIcon(id, icon)`, `reorder(ids)` | |
| `projects.attach(id, workspace)`, `detach(id)`, `delete(id)` | |
| `projects.next()`, `previous()` | Switches to the next or previous project in the sidebar |
| `projects.focus(id)` | Shows only that project in the sidebar; `null` shows them all again |
| `projects.sort(order)` | Sorts the sidebar: `manual`, `name`, `nameDescending`, `recentlyActive`, or `dateAdded` |
| `projects.removed()`, `restore(index)` | `[{ index, name, path, isLocal }]` of recently removed projects, and adds one back |
| `projects.apps()` | `[{ id, name }]`: the file manager and the editors **Open in** offers |
| `projects.openIn(id, app?)`, `reveal(id)` | Opens a project on this computer in an app (the default one without `app`), or shows it in the file manager |
| `workspaces.list()` | `[{ id, name, projectCount, isActive }]` |
| `workspaces.create(name)`, `switchTo(id)`, `rename(id, name)`, `delete(id)` | |
| `worktrees.list(project?)` | `[{ id, name, path, branch, isActive }]` |
| `worktrees.switchTo(id, project?)`, `refresh(project?)` | |
| `tabs.list({ project }?)` | `[{ index, id, kind, title, isActive, isPinned, color, customTitle, isDetached, paneIDs }]` for the current project, or the one named |
| `tabs.switchTo(indexOrIdOrTitle)`, `next()`, `previous()` | |
| `tabs.new({ beside, side }?)` | The new terminal tab's ID. With `beside`, it opens on the `left` or `right` (default) of that tab |
| `tabs.split(id, besideId, edge = "right")` | Moves a tab into a new [group](tabs.md#tab-groups) on the `left`, `right`, `up`, or `down` side of the group showing `besideId` |
| `tabs.open(request)` | The new tab's ID. See [Tabs](tabs.md#opening-tabs) |
| `tabs.close(id, { scope }?)` | Closes the tab, or with `scope` the `others`, or the ones to its `left` or `right`. Running processes ask first, as in the app |
| `tabs.rename(id, title)`, `setColor(id, color)` | Any tab. `null` resets; colors are palette names like `blue`, or hex |
| `tabs.pin(id, pinned = true)` | Pinned tabs sit first |
| `tabs.move(id, { index } \| { project })` | Reorders the tab, or moves it to the end of another project |
| `tabs.moveToWindow(id)`, `dock(id)` | Opens the tab in a window of its own, or returns it to the main window |
| `panes.list()` | `[{ id, title, workingDirectory, isFocused, tabID, projectID, worktreeID, isZoomed }]` of terminals across projects |
| `panes.send(id, text)`, `sendKeys(id, key)` | |
| `panes.readScreen(id, lines = 50)` | The last rows of the screen as text, up to 500 |
| `panes.close(id)`, `rename(id, title)` | |
| `panes.focus(id)` | Shows the pane's tab and focuses it |
| `panes.split(id, direction = "right")` | Splits the pane `left`, `right`, `up`, or `down`; returns the new pane's ID |
| `panes.zoom(id, zoomed?)` | Zooms or unzooms the pane; without `zoomed` it toggles |
| `panes.equalize(id)` | Makes the splits of the pane's tab equal |
| `panes.detach(id)`, `restart(id, { proxy }?)`, `scrollToBottom(id)` | Terminals only. A detached terminal keeps running |
| `panes.float(id, { scope }?)` | Floats the pane over the window, in its `project` (default) or `allProjects` |
| `panes.moveToTab(id, { project }?)`, `moveToWindow(id)` | Moves the pane into a new tab, or a window of its own |
| `agents.list()` | Agent status per worktree, like the `agent.status` event |
| `agents.focus(paneID)` | Jumps to an agent's pane |

IDs work across projects. `split` and `zoom` work on panes of the main window.
`send` and `sendKeys` only reach panes that are on screen. For a hidden pane, `readScreen` returns the text it last showed.

## App and window

| Call | Returns |
| --- | --- |
| `app.info()` | `{ name, version, platform, arch, language, theme: { name, colorScheme } }` |
| `app.openExternal(url)` | Opens an `http` or `https` link in the default browser |
| `ui.state()` | `{ sidebar, agentsPanel, agentSessions, composer, quickTerminal, floatsHidden, layout }` |
| `ui.setSidebar(visible?)`, `setAgentsPanel(visible?)`, `setAgentSessions(visible?)` | Shows or hides it; without `visible` it toggles |
| `ui.setLayout(layout)` | `project`, `tab`, or `agents` |
| `ui.openCommandPalette()`, `ui.openSettings(section = "general")` | `section` is `general`, `ai`, `composer`, `quickTerminal`, `appearance`, `terminal`, `keyboard`, `commands`, `browser`, `servers`, `extensions`, `backup`, or `about` |
| `ui.back()`, `ui.forward()` | Moves through tab history; `false` when there is nowhere to go |
| `ui.toggleFloats()` | Shows or hides every floating terminal |
| `ui.quickTerminal.show()`, `hide()`, `toggle()`, `pin(pinned = true)` | Fails while the Quick Terminal is turned off |

## Commands

`commands.list()` returns the app's built-in commands and the user's custom
commands from **Settings → Commands**:
`[{ id, kind, title, enabled, shortcut }]`, where `kind` is `app` or `custom`.
`commands.run(id)` runs one as the command palette would, and fails while it
isn't available. A custom command asks the user first, like opening a terminal
that runs a command. Quitting, hiding or resizing the app's windows, and
installing the command line tool are not offered.

```js
await muxy.commands.run("split_right");
```

## Composer

| Call | Does |
| --- | --- |
| `composer.open()`, `close()` | |
| `composer.getDraft()` | The draft's text |
| `composer.setDraft(text)` | Opens the composer and replaces its text |
| `composer.submit()`, `insert()` | Sends the draft as **Send** does, or without pressing Return as **Insert** does. Asks first, like typing into a terminal |

## Project layouts

`layouts.list({ project }?)` returns the project's
[layouts](../features/projects.md#layouts) as `[{ name, path }]`.
`layouts.apply(name, { project }?)` loads one and shows the app's own
**Apply Layout?** confirmation; it resolves once that is shown.

## Preferences

`preferences.get()` returns the app preferences extensions can change, and
`preferences.set(key, value)` changes one, as Settings would:

| Key | Value |
| --- | --- |
| `theme.light`, `theme.dark` | A name from `preferences.themes()` |
| `interfaceSize` | `regular`, `large`, `extraLarge`, or `huge` |
| `sidebar`, `statusBar`, `paneHeaders`, `tips`, `confirmProcess` | `true` or `false` |
| `terminal.fontSize` | A number |
| `terminal.fontFamily` | A font name |

## Settings

Read and change the extension's own [declared settings](manifest.md#settings).
Values the user hasn't set come back as their `defaultValue`.

```js
await muxy.settings.get("greeting");          // "Hello"
await muxy.settings.all();                    // { greeting: "Hello", loud: null }
await muxy.settings.set("greeting", "Hi");    // must match the declared type
await muxy.settings.reset("greeting");
```

Declare the `settings.changed` event to hear about changes, including ones
made in **Settings → Extensions**.

## Clipboard

`clipboard.readText()` returns the clipboard's text, or `null`; it asks the
user first. `clipboard.writeText(text)` replaces it.

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

`shortcuts.register({ id, combo })` binds a key while the extension runs.
Pressing it sends the `command.<id>` event to the extension. Commands in the
manifest use `defaultShortcut` instead. `combo` looks like `cmd+shift+e` or
`opt+left` and needs `cmd`, `ctrl`, or `opt`. `unregister(id)` and `list()`
manage them.

## GitHub

`gh.user()` returns the signed-in GitHub user from the `gh` CLI on the desktop
computer, even for remote projects. It is cached for five minutes.

## Page only

| Member | What it is |
| --- | --- |
| `extensionID` | The extension's name (everywhere) |
| `tabInstanceID`, `panelID` | This view's ID |
| `panelInstanceID` | A panel's [instance](panels.md#instances) name, or `null` |
| `panel` | The calling panel's frame and options. See [Panels](panels.md#from-the-panel-itself) |
| `data`, `onDataChange(fn)` | The view's data. See [Tabs](tabs.md#page-data) |
| `theme`, `onThemeChange(fn)` | Theme colors. See [Tabs](tabs.md#theme) |
| `focused`, `onFocus(fn)` | Whether the view has focus |
| `lifecycle.onBeforeClose(fn)` | Called before the view closes, with `{ surface, instanceID, panelInstanceID }`. Return `true`, or a Promise of it, to keep it open. A page that doesn't respond is closed after 5 s |
| `lifecycle.close()` | Closes this view |

More: [Events](events.md), [Tabs](tabs.md), [Panels](panels.md),
[Bar items](bar-items.md), [Dialogs](dialogs.md), [Files](files.md),
[Git](git.md).
