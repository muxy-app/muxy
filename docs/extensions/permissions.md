# Permissions

An extension lists what it needs in `muxy.permissions`. Calls without the
matching permission fail, and unknown permission names stop the extension from
loading. Ask for the minimum.

```json
"permissions": ["tabs:read", "git:read", "notifications:write"]
```

## Permissions

| Permission | Allows |
| --- | --- |
| `tabs:read` | `tabs.list` |
| `tabs:write` | Open, switch, split, and create tabs; set a tab's title and icon; `openTab` commands |
| `panes:read` | `panes.list`, `panes.readScreen` |
| `panes:write` | `panes.send`, `panes.sendKeys`, `panes.close`, `panes.rename` |
| `projects:read` | List projects and workspaces; the `projects.changed` event |
| `projects:write` | Add, switch, rename, recolor, reorder projects; manage workspaces |
| `projects:delete` | `projects.delete` |
| `worktrees:read` | `worktrees.list`; the `worktree.headChanged` event |
| `worktrees:write` | `worktrees.switchTo`, `worktrees.refresh` |
| `agents:read` | `agents.list`; the `agent.status` event |
| `files:read` | `files.list`, `files.read`, `files.stat`; the `file.changed` event |
| `files:write` | `files.write`, `mkdir`, `rename`, `move`, `delete` |
| `git:read` | Status, diffs, log, branches, worktrees, pull request info |
| `git:write` | Every other `git.*` call |
| `gh:read` | `gh.user` |
| `storage:read` / `storage:write` | Read / change the extension's storage |
| `notifications:write` | `notifications.notify`, `toast` |
| `panels:write` | Panels, popovers, modal web views, and setting bar items; panel, popover, and modal commands |
| `commands:run-script` | `runScript` commands |
| `commands:exec` | `exec`, `execAsync` |
| `shortcuts:register` | `shortcuts.register`, `shortcuts.unregister` |
| `browser:read` / `browser:write` | Read / drive [browser tabs](../features/browser.md) with `muxy.browser` |
| `remote:serve` | Accepted; remote methods are not available in 2.x |

`http.fetch`, dialogs, the picker, `events`, and `lifecycle` need no
permission.

## Runtime prompts

Some calls also ask the user each time, even with the permission:

| Asks before | Remembered per |
| --- | --- |
| Running a program (`exec`, `execAsync`) | Program, or shell command |
| Typing or pressing keys in a terminal | Extension |
| Reading terminal output | Extension |
| Opening a terminal tab that runs a command | Command |
| Opening another extension's tab | Extension |
| Changing the repository (`git:write` calls) | Operation |
| Changing files (`files:write` calls) | Operation |
| A web request (`http.fetch`) | Host |
| Deleting a project | Project |

The user can **Allow** once, **Allow & Remember**, **Cancel**, **Deny &
Remember**, or block that kind of action for the extension. An unanswered
prompt is denied after 60 seconds, and at most five prompts can wait at once.

The red **Always Allow This Extension…** button trusts the extension with
every action above, after a second warning confirms it. A trusted extension is
never prompted, and its saved rules, including Deny and Block, don't apply
until trust is removed. Cancelling the warning returns to the prompt.

Saved choices are listed under **Permission rules** on the extension's page in
**Settings → Extensions**, with **Reveal audit log** for a history of
decisions. **Always Allow…** there trusts the extension after the same warning,
and **Stop Always Allowing** removes the trust. **Clear all** and uninstalling
clear the rules and the trust.
