# AI Provider Hooks

The notification socket lets any sender include a `type` token in `type|paneID|title|body`. Built-in AI tools (Claude Code, Codex, Cursor, Droid, OpenCode, Pi) are mapped to their notification source by `AIProviderRegistry`. An extension can register a similar mapping for a third-party tool — no Muxy PR needed.

```json
{
  "aiProvider": {
    "socketTypeKey": "myagent",
    "displayName": "My Agent",
    "iconName": "sparkles"
  }
}
```

## What it does

When a notification arrives with `type = "myagent"`, `AIProviderRegistry.notificationSource(for:)` returns `.aiProvider("<extension-id>")` instead of `.socket`. The notification panel uses the declared `iconName` and treats the entry as the agent's own source.

```mermaid
flowchart LR
  Agent[Third-party agent CLI<br/>writes myagent|paneID|title|body] --> Sock[muxy.sock]
  Sock --> Server[NotificationSocketServer]
  Server -->|notificationSource type=myagent| Registry[AIProviderRegistry]
  Registry -->|no built-in match| Store[ExtensionStore<br/>declaredAIProvider]
  Store -->|extension declared| Source[.aiProvider extensionID]
  Source --> Panel[Notification panel<br/>renders with iconName]
```

## Fields

| Field | Required | Notes |
| --- | --- | --- |
| `socketTypeKey` | yes | The leading token used by your agent when writing to the socket (`<key>\|paneID\|title\|body`). |
| `displayName` | yes | Currently surfaced via the source's icon resolution; reserved for future use as the visible badge. |
| `iconName` | yes | SF Symbol name. Falls back to `sparkles` if unknown. |

## What it doesn't do

- It does **not** install the agent's hook script. You're responsible for getting your agent to send to `MUXY_SOCKET_PATH` with the right format.
- It does **not** add the agent to the AI providers list in **Settings → Notifications**. Built-in providers (with installable hooks) and extension-declared providers are distinct surfaces today.
- Built-in `socketTypeKey`s win. If you declare `claude`, the built-in `ClaudeCodeProvider` still routes that traffic.

## Sending a notification yourself

If your extension wants to post a notification directly (rather than just registering the routing for an external tool), open a second connection to the socket and write a single line. This path requires the [`notifications:write`](permissions.md) permission.

```bash
printf 'myagent|%s|Title|Body\n' "$MUXY_PANE_ID" | nc -U "$MUXY_SOCKET_PATH"
```

If `MUXY_PANE_ID` is empty, Muxy routes the notification to the first pane of the active worktree.
