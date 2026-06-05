# Permissions

Muxy enforces two layers on every extension call:

1. **Manifest permissions** — declared in `permissions`. Calling a verb without its permission returns `error:permission denied (<perm>)`.
2. **Runtime consent** — verbs that run code or touch terminal contents (`exec`, `panes.send`, `panes.sendKeys`, `panes.readScreen`) prompt the user even when the manifest permission is granted. The decision can be remembered as a rule.

Permissions apply only to identified callers. The host identifies itself on behalf of an extension; CLI clients (e.g. the `muxy` CLI) are unidentified and are not gated.

## Available permissions

| Permission | Grants |
| --- | --- |
| `panes:read` | `read-screen`, `list-panes` |
| `panes:write` | `split-right`, `split-down`, `send`, `send-keys`, `close-pane`, `rename-pane`. Split requests with a startup command also require `commands:exec`. |
| `tabs:read` | `list-tabs` |
| `tabs:write` | `switch-tab`, `new-tab`, `next-tab`, `previous-tab`, `open-tab` |
| `projects:read` | `list-projects` |
| `projects:write` | `switch-project` |
| `worktrees:read` | `list-worktrees` |
| `worktrees:write` | `create-worktree`, `switch-worktree`, `refresh-worktrees` |
| `git:read` | `git.status`, `git.diff`, `git.log`, `git.branches`, `git.currentBranch`, `git.aheadBehind`, `git.pr.info`, `git.pr.list`, `git.worktrees` — see [Git](git.md). |
| `git:write` | `git.stage`, `git.unstage`, `git.discard`, `git.commit`, `git.push`, `git.pull`, `git.branch.*`, `git.pr.*` (create/merge/close), `git.worktree.*`. Each call also prompts for runtime consent. |
| `files:read` | `files.list`, `files.read`, `files.stat` — see [Files](files.md). |
| `files:write` | `files.write`, `files.mkdir`, `files.rename`, `files.move`, `files.delete`. Each call also prompts for runtime consent. |
| `notifications:write` | `notifications.notify` (or its `toast` alias) to post a notification |
| `panels:write` | `panel.open`, `panel.toggle`, `panel.close` for declared [panels](panels.md); `popover.resize`, `popover.close` for the extension's open [popover](popovers.md). |
| `commands:run-script` | Execute `runScript` palette command actions in the per-extension JavaScriptCore context. |
| `commands:exec` | Run shell commands via `muxy.exec` (subprocess execution with stdout/stderr capture). |
| `remote:serve` | Serve [remote methods](remote-methods.md) declared in `remoteMethods` to the mobile app over the remote server. Each call also prompts for runtime consent. |

`muxy.http.fetch` ([HTTP](http.md)) needs **no manifest permission** — it is gated by host consent at runtime only.

## Runtime consent

These verbs prompt the user at runtime even when the manifest permission is granted:

| Verb | Reason |
| --- | --- |
| `exec` | Launching a subprocess on the user's machine. |
| `panes.send` | Typing arbitrary text into an active terminal. |
| `panes.sendKeys` | Pressing keys (including Ctrl+C, Enter) in an active terminal. |
| `panes.readScreen` | Reading the visible contents of a terminal. |
| `remote.invoke` | Running an extension's [remote method](remote-methods.md) handler in response to a mobile request. Remembered per action. |
| `git.*` (writes) | Mutating the repository (stage, commit, push, pull, branch, PR, worktree). Remembered per operation (allowing `push` does not allow `discard`). |
| `files.*` (writes) | Modifying workspace files (write, mkdir, rename, move, delete). Remembered per operation (allowing `write` does not allow `delete`). |
| `http.fetch` | Calling an external host via [`muxy.http`](http.md). Remembered per host (allowing `api.github.com` does not allow `example.com`). Private/loopback hosts are blocked before prompting. |

The prompt shows the extension, the verb, and the literal payload (full argv, the keystroke, or the pane id). The user picks:

- **Allow & remember** — runs the call and writes an allow rule.
- **Allow** — runs this one call, asks again next time.
- **Cancel** — denies this one call, asks again next time.
- **Deny & remember** — denies and writes a deny rule for that payload pattern.

Ticking **Block all … from this extension** before choosing **Deny & remember** writes a `blocked` rule for the whole verb, so the extension can never prompt for that kind again (e.g. blocking `exec` once stops all future command prompts, regardless of the command). It replaces any earlier rules for that verb, including allow rules.

A prompt left unanswered for 60 seconds is denied automatically.

Rules live in `~/Library/Application Support/Muxy/extension-grants.json` (Muxy-owned — extensions cannot self-grant). Every gated call appends to `~/Library/Application Support/Muxy/extension-audit.log` (`Settings → Extensions → Permissions → Reveal Audit Log`).

### Default "remember" patterns

| Verb | Pattern saved |
| --- | --- |
| `exec` (argv) | `argvPrefix` of the base command only. Allowing `git status` also allows other `git` subcommands. |
| `exec` (shell form) | `shellExact` of the full shell string. |
| `panes.*` / `tabs.openForeign` | `any` for that verb. Pane and tab targets are per-session, so the grant covers any future target. |
| `http.fetch` | `hostEquals` of the request host. Allowing `api.github.com` covers any path/method on that host but no other host. |

Rules can be reviewed, refined, or removed in `Settings → Extensions → Permissions`. Deny rules win over allow rules; more specific patterns win over less specific ones.

## What permissions don't gate

- **Subscribing to workspace events** is gated separately by the manifest `events` array — see [Events](events.md). The caller's identity, not a `permissions` entry, decides what it can subscribe to.
- **Extension-local events.** `muxy.events.subscribe('extension.*', ...)` and `muxy.events.emit('extension.*', payload)` stay inside the same extension, need no permission, and are not listed in the manifest.
- **Receiving palette command triggers.** Once an extension declares a command in `commands`, it can subscribe to its own `command.<id>` event without listing it under `events`.
- **Native dialogs.** `muxy.dialog.confirm` / `muxy.dialog.alert` present a sheet the user must dismiss — see [Dialogs](dialogs.md). Being user-driven and UI-only, they need no permission.
- **The modal picker.** `muxy.modal.open` presents a searchable picker the user drives to a selection or dismisses — see [Modal](modal.md). Being user-driven and UI-only, it needs no permission.

Permissions are coarse (verb groups, not individual verbs) on purpose while the API is in flux. Expect the list to expand and possibly split (e.g. `panes:send` vs `panes:close`) once a dedicated extension API layer lands.
