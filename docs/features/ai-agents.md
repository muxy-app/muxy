# AI Agents

Muxy recognizes AI coding agents running in its terminals and shows whether
each one is working, waiting for you, or done. It needs no setup, hooks, or
plugins, and works while no app is open.

## Supported agents

Antigravity, Claude Code, Codex, Copilot, Cursor, Droid, Grok, Kiro, OpenCode,
Pi, and Xal.

The server spots them from the running program and what's on screen.
Recognition is best effort.

## States

| State | Meaning |
| --- | --- |
| Working | The agent is busy. |
| Waiting | The agent needs your input. |
| Idle | The agent is done or not doing anything. |

Tabs, worktrees, and projects show the agent's icon and state. When several
agents run, "waiting" wins.

## Notifications

The desktop app sends a macOS notification when an agent needs your attention
or finishes working.

- No notification is shown for the pane you are looking at, or for detached
  terminals.
- Looking at the terminal clears the alert in every app.
- Clicking a notification jumps to its pane.
- Only one app notifies, so alerts are never doubled.
- Turn notifications off in **System Settings → Notifications → Muxy**.

Alerts live in memory on the server, at most one per terminal. Restarting the
server clears them.

## Agents Focused sidebar

Choose **Agents Focused** from the sidebar layout menu to list only the tabs
that run an agent.

## From a shell and extensions

- `muxy activity list --json` prints agent status and unread alerts as JSON.
  `muxy activity ack <event-id>` clears alerts.
- Extensions get the `agent.status` event and `muxy.agents.list()`. See
  [Events](../extensions/events.md).
