# Notification Setup

Muxy already ships built-in integrations for **Claude Code** and **OpenCode** — toggle them under **Settings → Notifications** and you're done.

This document is for everything else: sending notifications into Muxy from **any other tool** (a custom CLI, a shell command, a build script, a different AI agent, etc.).

## How Muxy Receives Notifications

Muxy listens on a Unix domain socket:

```
~/Library/Application Support/Muxy/muxy.sock
```

The socket path is also exported to every terminal Muxy spawns as the environment variable `MUXY_SOCKET_PATH`, along with a per-pane identifier `MUXY_PANE_ID`. Any process running inside a Muxy terminal pane can read these and send a message.

## Wire Format

One message per connection. The payload is a single UTF-8 line with four pipe-separated fields:

```
<type>|<paneID>|<title>|<body>
```

| Field    | Required | Description                                                                 |
| -------- | -------- | --------------------------------------------------------------------------- |
| `type`   | yes      | Identifier for the source. Unknown values are accepted and shown generically. Built-in values: `claude_hook`, `opencode`. |
| `paneID` | yes      | The pane the event belongs to. Use `$MUXY_PANE_ID` when sending from inside a Muxy terminal. Leave empty to attach the notification to the currently active pane. |
| `title`  | yes      | Shown as the notification title. If empty, Muxy uses `Task completed!`.     |
| `body`   | no       | Notification body. Must not contain `\|` or newlines — replace them first.   |

Constraints:

- Max message size: **64 KB**.
- The `|` character is the field separator — strip or replace it in user-supplied strings.
- Newlines terminate a message; you can send multiple messages on one connection by separating them with `\n`.

## Minimal Example — Shell

From anywhere inside a Muxy terminal pane:

```bash
printf '%s|%s|%s|%s' \
    "custom" "$MUXY_PANE_ID" "Build finished" "All tests passed" \
    | nc -U "$MUXY_SOCKET_PATH"
```

Wrap it in a function and call it from anywhere:

```bash
muxy_notify() {
    [ -z "${MUXY_SOCKET_PATH:-}" ] && return 0
    local title="${1:-Done}"
    local body="${2:-}"
    local safe_body
    safe_body=$(printf '%s' "$body" | tr '|\n\r' '   ' | head -c 500)
    printf '%s|%s|%s|%s' "custom" "${MUXY_PANE_ID:-}" "$title" "$safe_body" \
        | nc -U "$MUXY_SOCKET_PATH" 2>/dev/null || true
}

# Usage
long-running-build && muxy_notify "Build finished" "main @ $(git rev-parse --short HEAD)"
```

## Minimal Example — Node.js

```javascript
import { createConnection } from "net"

function muxyNotify(title, body = "") {
  const socketPath = process.env.MUXY_SOCKET_PATH
  const paneID = process.env.MUXY_PANE_ID || ""
  if (!socketPath) return
  const safeBody = String(body).replace(/[\n\r|]+/g, " ").slice(0, 500)
  const payload = `custom|${paneID}|${title}|${safeBody}`
  const conn = createConnection({ path: socketPath })
  conn.on("error", () => {})
  conn.write(payload, () => conn.end())
}
```

## Minimal Example — Python

```python
import os, socket

def muxy_notify(title: str, body: str = "") -> None:
    path = os.environ.get("MUXY_SOCKET_PATH")
    pane = os.environ.get("MUXY_PANE_ID", "")
    if not path:
        return
    safe_body = body.replace("|", " ").replace("\n", " ")[:500]
    payload = f"custom|{pane}|{title}|{safe_body}".encode("utf-8")
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
        s.connect(path)
        s.sendall(payload)
```

## Reference Implementations

The built-in integrations are good templates for writing your own:

- **Shell hook (Claude Code):** [`scripts/muxy-claude-hook.sh`](../scripts/muxy-claude-hook.sh)
- **Node plugin (OpenCode):** [`scripts/opencode-muxy-plugin.js`](../scripts/opencode-muxy-plugin.js)

## Tips

- **Fire and forget.** If Muxy isn't running or the socket doesn't exist, the connection will fail — swallow the error rather than crashing your tool. Every example above does this.
- **Don't block.** Open the connection, write the payload, close it. Do not wait for a response — Muxy doesn't send one.
- **Sanitize.** Always strip `|`, `\n`, `\r` from user/model-generated content before sending, and cap the body length (200–500 characters is plenty).
- **Pane routing.** If you send from outside a Muxy pane (e.g. a cron job), omit `paneID`; Muxy will route to the currently active pane of the active project.
- **Type strings.** Pick something descriptive for `type`. If it doesn't match a registered provider, Muxy still shows the notification with a generic source — your `title` field is what users actually see.

## Delivery Settings

Regardless of where a notification comes from, Muxy respects the user's choices under **Settings → Notifications**:

- **Toast** — show an in-app banner
- **Sound** — play a system sound on arrival
- **Position** — where the toast appears

A dot also appears on the project and worktree rows in the sidebar until the notification is read.
