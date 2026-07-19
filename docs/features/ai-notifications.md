# AI notifications

Muxy tracks the AI coding agents running inside its terminals — Claude Code, Codex, Cursor, Droid, Grok, OpenCode, and Pi — and surfaces their lifecycle as pane and worktree status, completion badges, and notifications when a turn finishes or an agent needs attention.

There are two independent sources of truth, and hooks are authoritative.

## Detection vs. hooks

- **Hooks** are the primary signal. Each provider's CLI is configured to run a small Muxy hook when it starts a turn, finishes, or needs input. The hook reports the exact lifecycle phase (`working`, `waiting`, `finished`), so status changes are precise and event-driven.
- **Detection** is the fallback. Muxy watches the foreground process of each pane to notice when an agent is running even if its hooks are missing or misconfigured. Detection can tell that an agent *stopped* being the foreground process, but not why.

When detection reports that a working agent is no longer active, Muxy waits a short grace window (4 seconds) for a hook `finished` event before falling back to marking the pane idle. A hook event arriving inside the window always wins, so a correctly hooked agent is never idled prematurely by detection.

## Protocol (v3)

Hooks talk to Muxy over a Unix domain socket at `~/Library/Application Support/Muxy/muxy.sock` (`muxy-dev.sock` for debug builds). The wire format is a single newline-delimited JSON object per event, acknowledged by the server:

```json
{"v":3,"kind":"agent_event","provider":"claude_hook","paneID":"…","phase":"finished","title":"Claude Code","body":"Done","pids":[],"ts":1721234567}
```

- `paneID` is the target pane. When the CLI cannot know it, the field is omitted and `pids` carries the process's ancestor chain; Muxy resolves the nearest matching pane by foreground process id.
- `phase` is `working`, `waiting`, or `finished`. `finished` maps to the `idle` status.
- `test: true` marks a synthetic event from the settings Test button — it is delivered as a notification but never changes agent status.

The server replies with `{"v":3,"kind":"ack","ok":true}`. Events with the wrong version, wrong kind, empty provider, or a malformed pane id are rejected without an ack. This is the only agent protocol Muxy accepts; there is no pipe-format fallback.

## Staging layout

The compiled hook bridge (`muxy-hook`) and the provider shims are staged into `~/Library/Application Support/Muxy/hooks` (`hooks-dev` for debug builds) with private permissions:

- `muxy-hook` — the compiled bridge every hook invokes.
- `muxy-claude-hook.sh`, `muxy-codex-hook.sh`, `muxy-cursor-hook.sh`, `muxy-droid-hook.sh`, `muxy-grok-hook.sh` — thin shell shims that exec the colocated `muxy-hook`.
- `opencode-muxy-plugin.js`, `muxy-pi-extension.ts` — plugin/extension entry points that spawn the staged `muxy-hook`. When the binary is missing they log a clear error and skip the event; the health engine restages it.

Terminals export `MUXY_PANE_ID`, `MUXY_SOCKET_PATH`, and `MUXY_HOOK_BIN` so shims and plugins can reach the socket and binary.

## Health and repair engine

Each provider integration is reconciled declaratively: Muxy **verifies** the provider's hook configuration against what it should be and **repairs** it in place when it drifts, preserving any foreign hooks the user configured. Reconciliation runs at launch, when a provider becomes available, and whenever a provider's config file changes — the latter is watched with FSEvents and debounced, so an external edit to `~/.claude`, `~/.codex`, and the like triggers an automatic re-verify without polling.

Results are tracked per provider in the health store — install state, last verified/repaired time, last event time, and last error — and shown in **Settings → Notifications** as a status dot and line per provider. A `conflict` means Muxy found a non-Muxy hook it will not overwrite; the message names it.

## Test button

Each provider row in **Settings → Notifications** has a **Test** button. It runs the staged `muxy-hook` with `--event test --test`, which sends a `test: true` event over the live socket. A passing test confirms the full path end to end — staged binary, socket, server, ack, and notification delivery — without touching agent status. **Refresh** restages the provider's hook files and re-runs reconciliation.

## Extension surface

Agent status is exposed to extensions as the `agent.status` event and `muxy.agents.list()`, and completions post `notification.posted`. See [Extension events](../extensions/events.md).

## Troubleshooting

- **No notifications from an agent.** Open **Settings → Notifications**, check the provider's status dot, and click **Refresh** to restage and re-verify. Run **Test** to confirm the socket path end to end.
- **Hook delivery failures.** The bridge logs failures to `~/Library/Application Support/Muxy/hooks.log`.
- **Socket missing.** Verify it exists: `ls -l ~/Library/Application\ Support/Muxy/muxy.sock`.
- **Conflict reported.** Muxy found a foreign hook in the provider's config and left it untouched. Remove or rename it if you want Muxy to own that hook, then **Refresh**.
- **Logs.** Stream live: `log stream --predicate 'subsystem == "app.muxy"' --info --debug`.

See also [Terminal notifications](terminal.md) for OSC-based terminal notifications, and the general [Troubleshooting](../user-guide/troubleshooting.md) guide.
