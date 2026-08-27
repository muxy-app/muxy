# Socket protocol

Muxy P2 preserves the retained 1.x Unix socket contract while keeping transport policy separate from app behavior. The portable transport lives in `muxy-proto`; command recognition, permission decisions, state mutation, terminal targeting, and non-visual ingress sinks live in the `muxy` app.

## Endpoints and environment

| Build | Socket filename |
|---|---|
| Debug | `muxy-dev.sock` |
| Release | `muxy.sock` |

The filename is selected from the build mode in `muxy-core::environment`. Runtime environment variables cannot change debug into release or authorize shared-profile mutation.

Every spawned P2 terminal receives:

| Variable | Value |
|---|---|
| `MUXY_PROJECT_ID` | Uppercase project UUID |
| `MUXY_WORKTREE_ID` | Uppercase worktree UUID |
| `MUXY_PANE_ID` | Uppercase pane UUID |
| `MUXY_SOCKET_PATH` | Selected absolute socket path |

`MUXY_HOOK_BIN` and `MUXY_HOOK_SCRIPT` are not set until P11.

The retained CLI honors `MUXY_SOCKET_PATH`. The installed shim honors `MUXY_APP_PATH` and resolves the nested executable resource at `Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli`.

Debug bundles also contain `Contents/Resources/muxy-dev-bin/muxy`. Every newly created debug terminal prepends that directory to `PATH` and receives the current bundle path, development socket, and package version. Normal commands forward unchanged to the retained bundled CLI. `muxy version`, `muxy --version`, and `muxy -V` report the exact debug app, CLI, socket, and socket status. Release bundles do not contain or inject this launcher. Existing terminals must be reopened after rebuilding because a running shell's environment cannot be replaced.

Start the debug app with `scripts/run.sh debug`. Inside any terminal created afterward, use `muxy version` to confirm the selected app, CLI, and socket, then run commands such as `muxy list-projects` without additional environment exports.

## Transport and framing

The server accepts newline-delimited input records with a 128 KiB unread-buffer limit. Input may span reads or contain multiple records. Routing trims surrounding Unicode whitespace for matching while preserving the original record for payload-sensitive routes.

| Session class | Reply framing | Lifetime |
|---|---|---|
| Unidentified CLI | Raw UTF-8 followed by `0x00` | Closes after the accepted reply flushes |
| Identified extension | UTF-8 followed by `\n` | Persistent |
| Agent Hook v3 | `{"kind":"ack","ok":true,"v":3}\n` | May close after acknowledgement |
| One-way ingress | No bytes | Caller may close |

A session accepts at most eight concurrent app commands. An additional command received while eight replies are in flight gets `error:too many concurrent commands`. Capacity returns when a command completes. The framing selected when an app request is accepted stays attached to that request even if an extension snapshot changes before app completion. Read EOF blocks new pushes and invokes but does not discard an already owed app reply or a complete buffered invoke result.

The Unix listener:

- creates parent directories without weakening existing permissions
- binds with socket mode 0600
- proves the bound path reaches the new listener before claiming ownership
- refuses a live endpoint without unlinking it
- removes only a conclusively stale socket
- removes the socket on shutdown only if path identity still matches the owned inode
- avoids process-wide SIGPIPE mutation

## Routing order

Each complete record follows this order:

1. If the session is unidentified, attempt strict Agent Hook v3 JSON parsing.
2. A valid hook is acknowledged before deduplication and app delivery.
3. Handle transport-owned sticky commands: `identify` and `subscribe`.
4. Route a recognized app-command head to the app dispatcher.
5. For an identified session, handle `invoke-result` and incoming `extension-event`.
6. Route `open-project` and `install-extension` as no-response compatibility ingress.
7. Parse a structurally valid legacy notification with max-three-split payload behavior.
8. Ignore records that match no route.

Known deferred app heads remain recognized and return deterministic `error:*` replies. They do not fall through into notification parsing.

## Extension sessions

`identify|<extension-id>|<token>` requires a loaded extension ID and an exact non-empty token. The newest identified session becomes the live invoke and targeted-push owner for that extension. Disconnecting that session does not fall back to an older session until the older session identifies again.

`subscribe|<event>` is allowed before identification. Identified subscriptions require the event to be declared and readable in the applied snapshot. Snapshot replacement filters subscriptions, clears removed identities, removes their live mappings, and fails their pending invokes.

Identified app requests carry an immutable origin containing the extension ID and current permission grants. The app checks P2 permissions before mutation. Split commands with a non-empty startup command require both `panes:write` and `commands:exec`.

Valid identified legacy notifications require notification write access. A denied valid notification increments the session drop count. The session disconnects on the 100th denied notification. Malformed records and allowed notifications do not increment that count.

Outbound extension frames are typed forms for broadcast `event`, targeted `extension-event`, modal result, modal query, and invoke. Invokes are owned by one session and complete exactly once with success, decoded error, unavailable, or timeout. Timeout is 15 seconds.

## Agent Hook v3

A hook is a strict JSON envelope with `v: 3`, `kind: "agent_event"`, a supported phase, required provider/title/body/PID/timestamp fields, an optional canonical pane UUID, optional ID, and optional test flag. The acknowledgement is sent before the server checks its 256-entry recent-ID set.

The app records a typed resolution:

1. test event
2. explicit pane
3. first supplied PID with a matching terminal foreground PID
4. unresolved

When one PID matches multiple panes, uppercase pane-ID order breaks the tie. Hook, extension-event, and legacy-notification app queues are bounded and evict their oldest record on overflow. P2 intentionally adds no notification UI.

## Direct pipe surface

These are the 34 accepted P2/P3 wire heads. Every row is implemented and reachable through the untouched wrapper.

| Wrapper invocation | Direct head | App permission |
|---|---|---|
| `muxy split-right ...` | `split-right` | `panes:write`, plus `commands:exec` for startup commands |
| `muxy split-down ...` | `split-down` | `panes:write`, plus `commands:exec` for startup commands |
| `muxy send ...` | `send` | `panes:write` |
| `muxy send-keys ...` | `send-keys` | `panes:write` |
| `muxy read-screen ...` | `read-screen` | `panes:read` |
| `muxy close-pane ...` | `close-pane` | `panes:write` |
| `muxy rename-pane ...` | `rename-pane` | `panes:write` |
| `muxy list-panes` | `list-panes` | `panes:read` |
| `muxy list-projects` | `list-projects` | `projects:read` |
| `muxy switch-project ...` | `switch-project` | `projects:write` |
| `muxy list-worktrees ...` | `list-worktrees` | `worktrees:read` |
| `muxy switch-worktree ...` | `switch-worktree` | `worktrees:write` |
| `muxy refresh-worktrees ...` | `refresh-worktrees` | `worktrees:write` |
| `muxy create-worktree ...` | `create-worktree` | `worktrees:write` |
| `muxy list-workspaces` | `list-workspaces` | `projects:read` |
| `muxy create-workspace ...` | `create-workspace` | `projects:write` |
| `muxy switch-workspace ...` | `switch-workspace` | `projects:write` |
| `muxy rename-workspace ...` | `rename-workspace` | `projects:write` |
| `muxy delete-workspace ...` | `delete-workspace` | `projects:write` |
| `muxy create-project ...` | `create-project` | `projects:write` |
| `muxy attach-project ...` | `attach-project` | `projects:write` |
| `muxy detach-project ...` | `detach-project` | `projects:write` |
| `muxy list-tabs` | `list-tabs` | `tabs:read` |
| `muxy switch-tab ...` | `switch-tab` | `tabs:write` |
| `muxy new-tab` | `new-tab` | `tabs:write` |
| `muxy next-tab` | `next-tab` | `tabs:write` |
| `muxy previous-tab` | `previous-tab` | `tabs:write` |
| `muxy tab rename ...` | `tab-rename` | `tabs:write` |
| `muxy tab set-color ...` | `tab-set-color` | `tabs:write` |
| `muxy tab set-icon ...` | `tab-set-icon` | `tabs:write` |
| `muxy tab pin ...` | `tab-pin` | `tabs:write` |
| `muxy tab unpin ...` | `tab-unpin` | `tabs:write` |
| `muxy tab close ...` | `tab-close` | `tabs:write` |
| `muxy tab move ...` | `tab-move` | `tabs:write` |

`muxy <existing-directory>` sends the outside-count `open-project` one-way route.

Target-aware split and tab commands accept trailing `--project` and `--worktree` pairs. Project identifiers match UUID, case-insensitive name, or standardized path. Worktrees match UUID, case-insensitive name or branch, or standardized path. A worktree-only match prefers the active project and otherwise must be unique across projects.

## Recognition status and owners

The app recognition catalog contains exactly 169 heads.

| Direct category | Count | Status and owner |
|---|---:|---|
| P2 aliases listed above | 33 | Implemented in P2 |
| `create-worktree` | 1 | Implemented in P3 |
| `list-sessions`, `kill-session` | 2 | Recognized, deferred to P8 |
| `open-tab` | 1 | Recognized, deferred to P10 |
| Browser heads | 36 | Recognized, deferred to P9 |
| Extension/API heads | 96 | Recognized, deferred to P10 |

The 36 P9 browser heads are:

`browser.open`, `browser.navigate`, `browser.list`, `browser.read`, `browser.close`, `browser.eval`, `browser.click`, `browser.type`, `browser.waitFor`, `browser.getText`, `browser.getHTML`, `browser.getAttribute`, `browser.reload`, `browser.back`, `browser.forward`, `browser.waitForNavigation`, `browser.screenshot`, `browser.storage.get`, `browser.storage.set`, `browser.storage.clear`, `browser.cookies.get`, `browser.cookies.set`, `browser.cookies.delete`, `browser.cookies.clear`, `browser.wait`, `browser.fill`, `browser.press`, `browser.select`, `browser.hover`, `browser.scrollIntoView`, `browser.setChecked`, `browser.is`, `browser.getValue`, `browser.getCount`, `browser.find`, `browser.snapshot`.

The 96 P10 extension/API heads are:

`exec`, `agents.list`, `http.fetch`, `dialog.confirm`, `dialog.alert`, `dialog.prompt`, `dialog.pickFolder`, `shortcuts.register`, `shortcuts.unregister`, `shortcuts.list`, `storage.get`, `storage.set`, `storage.delete`, `storage.keys`, `modal.open`, `modal.feed`, `modal.finish`, `modal.await`, `modal.openWebview`, `modal.awaitWebview`, `modal.submitWebview`, `modal.closeWebview`, `extension.settings.get`, `extension.settings.set`, `extension.statusbar.set`, `panel.open`, `panel.close`, `panel.toggle`, `popover.close`, `popover.resize`, `topbar.set`, `statusbar.set`, `tabs.open`, `projects.delete`, `projects.add`, `projects.create`, `projects.rename`, `projects.setColor`, `projects.setIcon`, `projects.setLogo`, `projects.reorder`, `projects.attach`, `projects.detach`, `workspaces.list`, `workspaces.create`, `workspaces.switch`, `workspaces.rename`, `workspaces.delete`, `lifecycle.ackBeforeClose`, `lifecycle.resolveBeforeClose`, `lifecycle.closeSelf`, `files.list`, `files.read`, `files.stat`, `files.write`, `files.mkdir`, `files.rename`, `files.move`, `files.delete`, `git.status`, `git.diff`, `git.repoInfo`, `git.log`, `git.branches`, `git.currentBranch`, `git.aheadBehind`, `git.pr.info`, `git.pr.number`, `git.pr.diff`, `git.pr.list`, `git.worktrees`, `git.init`, `git.stage`, `git.unstage`, `git.discard`, `git.commit`, `git.push`, `git.pull`, `git.branch.create`, `git.branch.switch`, `git.pr.create`, `git.pr.merge`, `git.pr.close`, `git.worktree.add`, `git.worktree.remove`, `git.worktree.switch`, `git.remoteBranches`, `git.branch.delete`, `git.branch.deleteRemote`, `git.checkout`, `git.cherryPick`, `git.revert`, `git.tag.create`, `git.pr.checkout`, `git.pr.checkoutWorktree`, `gh.user`.

## Dispatcher execution surface

The retained canonical `MuxyAPIDispatcher` execution surface contains 146 names. It contains 126 names shared with the 132 browser and extension/API heads after excluding these outside-dispatcher names:

`extension.settings.get`, `extension.settings.set`, `extension.statusbar.set`, `lifecycle.ackBeforeClose`, `lifecycle.resolveBeforeClose`, `lifecycle.closeSelf`.

It additionally contains these 20 dispatcher-only canonical names:

`toast`, `notifications.notify`, `panes.close`, `panes.list`, `panes.readScreen`, `panes.rename`, `panes.send`, `panes.sendKeys`, `projects.list`, `projects.switch`, `tabs.list`, `tabs.new`, `tabs.next`, `tabs.previous`, `tabs.setIcon`, `tabs.setTitle`, `tabs.switch`, `worktrees.list`, `worktrees.refresh`, `worktrees.switch`.

Canonical dispatcher names are documentation and permission-mapping inputs. They are not automatically direct socket heads.

## Outside-dispatcher surface

The retained 15-name canonical outside-dispatcher surface is:

`panes.split`, `sessions.list`, `sessions.kill`, `worktrees.create`, `tabs.rename`, `tabs.setColor`, `tabs.setPin`, `tabs.close`, `tabs.move`, `extension.settings.get`, `extension.settings.set`, `extension.statusbar.set`, `lifecycle.ackBeforeClose`, `lifecycle.resolveBeforeClose`, `lifecycle.closeSelf`.

P2 implements backing behavior through its accepted direct aliases. P3 implements `worktrees.create` behavior through the retained `create-worktree` alias. P8 owns sessions, and P10 owns extension settings and lifecycle APIs.

The following transport or ingress forms are outside the 169 app-command count:

| Form | Direction and status |
|---|---|
| `identify`, `subscribe` | Incoming transport-owned sticky commands, implemented in P2 |
| `invoke-result` | Incoming identified extension completion, implemented in P2 |
| `extension-event` | Bidirectional identified local event, implemented in P2 |
| `open-project` | Incoming one-way path open, implemented in P2 |
| `install-extension` | Incoming one-way compatibility no-op, implementation deferred to P10 |
| Agent Hook v3 JSON | Incoming acknowledgement plus typed sink, implemented in P2 |
| Legacy `type|paneID|title|body` | Incoming one-way typed sink, implemented in P2; UI deferred to P5 |
| `invoke`, `modal-result`, `modal-query`, `event` | Outbound extension pushes, transport implemented in P2 |
| `config-export`, `config-import` | Retained recognition oddity, handler-like wrapper forms not present in the 169-name recognition set; deferred to P14 |
| `install-skills` | Executes locally in the CLI; deferred to P14 |

## Portability boundary

`muxy-proto` owns portable framing, strict extension and hook codecs, request/reply types, session limits, snapshot mechanics, invoke correlation, and the Unix listener. It has no dependencies on GPUI, Objective-C, Ghostty, `muxy-core`, `muxy-api`, `muxy-terminal`, or `muxy-ui`.

The app owns all command names, phase ownership, permissions, project/worktree/tab/pane resolution, persistence, foreground PID lookup, and GPUI lifecycle. Unix listener implementations are compiled on macOS and Linux. Unsupported platforms expose a typed unsupported server rather than importing app policy into the protocol crate.

## Explicit P2 exclusions

P2 does not implement browser commands, session commands, extension APIs, hook installation or client resources, notification UI, backup/config behavior, skill installation, URL registration or cold launch, or production profile hardening. P3 adds the retained `create-worktree` alias without changing the 169-head recognition inventory. The remaining work stays assigned to P5, P8, P9, P10, P11, P14, P15, and P2.5 as listed above.
