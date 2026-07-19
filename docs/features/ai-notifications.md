# AI notifications

Muxy tracks the AI coding agents running inside its terminals — Claude Code, Codex, Cursor, Droid, Grok, OpenCode, and Pi — and surfaces their lifecycle as pane and worktree status, completion badges, and notifications when a turn finishes or an agent needs attention.

There are two independent sources of truth, and hooks are authoritative.

## Detection vs. hooks

- **Hooks** are the primary signal. Each provider's CLI is configured to run a small Muxy hook when it starts a turn, finishes, or needs input. The hook reports the exact lifecycle phase (`working`, `waiting`, `finished`), so status changes are precise and event-driven.
- **Detection** is the fallback. Muxy watches the foreground process of each pane to notice when an agent is running even if its hooks are missing or misconfigured. Detection can tell that an agent *stopped* being the foreground process, but not why.

When detection reports that a working agent is no longer active, Muxy waits a short grace window (4 seconds) for a hook `finished` event before falling back to marking the pane idle. A hook event arriving inside the window always wins, so a correctly hooked agent is never idled prematurely by detection.

A `waiting` pane is handled more conservatively, because a waiting agent still has a live process and must never be idled just for leaving the foreground. It uses a much longer window (30 seconds), and Muxy records the detected process ID before detection is lost. At the end of each window, Muxy idles the pane only if that process is gone; a live process schedules another low-frequency check. This recovers panes whose agent was killed before its `Stop` hook could run, without cutting short an agent that is genuinely waiting on input.

## Protocol (v3)

Hooks talk to Muxy over a Unix domain socket at `~/Library/Application Support/Muxy/muxy.sock` (`muxy-dev.sock` for debug builds). The wire format is a single newline-delimited JSON object per event, acknowledged by the server:

```json
{"v":3,"kind":"agent_event","id":"…","provider":"claude_hook","paneID":"…","phase":"finished","title":"Claude Code","body":"Done","pids":[],"ts":1721234567}
```

- `id` is a UUID identifying one logical event. The bridge generates it once when the message is built and re-sends the identical line on every retry, so a retried event carries the same `id`.
- `paneID` is the target pane. When the CLI cannot know it, the field is omitted and `pids` carries the process's ancestor chain; Muxy resolves the nearest matching pane by foreground process id.
- `phase` is `working`, `waiting`, or `finished`. `finished` maps to the `idle` status.
- `test: true` marks a synthetic event from the settings Test button — it is delivered as a notification but never changes agent status.

The server replies with `{"v":3,"kind":"ack","ok":true}`. Events with the wrong version, wrong kind, empty provider, or a malformed pane id are rejected without an ack. This is the only agent protocol Muxy accepts; there is no pipe-format fallback.

The bridge accepts at most 1 MiB from standard input and uses one monotonic 400 ms execution budget across input reading, socket connection, writes, acknowledgement reads, and retries. Reaching the payload cap returns immediately instead of draining the rest of standard input.

### Duplicate suppression

The bridge retries an event when an ack does not arrive within its delivery budget, so a lost or slow ack can deliver the same event twice. The server remembers the 256 most recently applied `id` values (FIFO eviction) and, on a repeat, still acks so the client stops retrying, then skips the event entirely. Duplicate deliveries do not update agent status, hook health, event time, or notifications. An event with a missing or empty `id` is never deduplicated and is always delivered.

## Staging layout

The compiled hook bridge (`muxy-hook`) and the provider shims are staged into `~/Library/Application Support/Muxy/hooks` (`hooks-dev` for debug builds) with private permissions:

- `muxy-hook` — the compiled bridge every hook invokes.
- `muxy-claude-hook.sh`, `muxy-codex-hook.sh`, `muxy-cursor-hook.sh`, `muxy-droid-hook.sh`, `muxy-grok-hook.sh` — thin shell shims that exec the colocated `muxy-hook`.
- `opencode-muxy-plugin.js`, `muxy-pi-extension.ts` — plugin/extension entry points that spawn the staged `muxy-hook`. When the binary is missing they log a clear error to their own stderr and skip the event. That stderr never reaches Muxy, so nothing restages automatically — use **Refresh** in Settings to restage.

Reconciliation starts only after the complete staged resource set is available. Each provider also verifies that the shared `muxy-hook` bridge exists and is executable, so a stale shim or plugin cannot report healthy while its bridge is missing.

Terminals export `MUXY_PANE_ID`, `MUXY_SOCKET_PATH`, `MUXY_HOOK_BIN`, `MUXY_HOOK_SCRIPT`, `MUXY_PROJECT_ID`, and `MUXY_WORKTREE_ID` so shims and plugins can reach the socket and binary and identify their context.

## Health and repair engine

Each provider integration is reconciled declaratively: Muxy **verifies** that every managed event has exactly one current Muxy hook and **repairs** stale or duplicate entries in place when it drifts, preserving any foreign hooks the user configured. Reconciliation runs at launch, when a provider becomes available, and whenever a provider's config file changes — the latter is watched with FSEvents and debounced, so an external edit to `~/.claude`, `~/.codex`, and the like triggers an automatic re-verify without polling.

Muxy records a hash of every config file it writes and ignores watcher events whose content matches its own last write, so a repair never re-triggers itself. A per-file rate limiter caps repairs within a rolling minute; when it trips — most commonly when a release and a debug build both manage the same config — Muxy stops rewriting and reports a `conflict` instead of spinning.

Results are tracked per provider in the health store — install state, last verified/repaired time, last event time, and last error — and shown in **Settings → Notifications** as a status dot and line per provider. A `conflict` means Muxy found a non-Muxy hook it will not overwrite; the message names it.

## Test button

Each provider row in **Settings → Notifications** has a **Test** button. It runs the staged `muxy-hook` with `--event test --test`, which sends a `test: true` event over the live socket. The pass/fail signal is the bridge's exit code, so a passing test confirms the delivery path — staged binary, socket, server, and ack — without touching agent status. It does not verify that the resulting in-app notification was presented. **Refresh** restages the provider's hook files and re-runs reconciliation.

## Extension surface

Agent status is exposed to extensions as the `agent.status` event and `muxy.agents.list()`, and completions post `notification.posted`. See [Extension events](../extensions/events.md).

## Troubleshooting

- **No notifications from an agent.** Open **Settings → Notifications**, check the provider's status dot, and click **Refresh** to restage and re-verify. Run **Test** to confirm the socket path end to end.
- **Hook delivery failures.** The bridge logs failures to `~/Library/Application Support/Muxy/hooks.log`.
- **Socket missing.** Verify it exists: `ls -l ~/Library/Application\ Support/Muxy/muxy.sock`.
- **Conflict reported.** Muxy found a foreign hook in the provider's config and left it untouched. Remove or rename it if you want Muxy to own that hook, then **Refresh**.
- **Logs.** Stream live: `log stream --predicate 'subsystem == "app.muxy"' --info --debug`.

See also [Terminal notifications](terminal.md) for OSC-based terminal notifications, and the general [Troubleshooting](../user-guide/troubleshooting.md) guide.
