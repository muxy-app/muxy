# AI Provider Hooks

External agent CLIs report activity to Muxy by writing notifications to its socket, each tagged with a `type` token. Built-in agents (Claude Code, Codex, Cursor, Droid, OpenCode, Pi) are mapped to their source by `AIProviderRegistry`. Declaring an `aiProvider` lets an extension register the same kind of mapping for a third-party agent — no Muxy PR needed.

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

When a notification arrives with `type = "myagent"`, Muxy resolves its source to `.aiProvider("<extension-id>")` instead of the generic socket source, and the notification panel renders it with the declared `iconName`.

```mermaid
flowchart LR
  Agent[Agent CLI writes myagent notification] --> Server[NotificationSocketServer]
  Server --> Registry[AIProviderRegistry]
  Registry -->|extension declared| Source[.aiProvider extensionID]
  Source --> Panel[Notification panel renders with iconName]
```

## Fields

| Field | Required | Notes |
| --- | --- | --- |
| `socketTypeKey` | yes | The leading token your agent writes (`<key>\|paneID\|title\|body`). |
| `displayName` | yes | Surfaced via the source's icon resolution; reserved for future use as the visible badge. |
| `iconName` | yes | SF Symbol name. Falls back to `sparkles` if unknown. |

## What it doesn't do

- It does **not** install the agent's hook. You're responsible for getting your agent to write to the socket in the right format.
- It does **not** add the agent to **Settings → Notifications**. Built-in providers (with installable hooks) and extension-declared providers are separate surfaces today.
- Built-in `socketTypeKey`s win — declaring `claude` still routes to the built-in provider.

## Sending a notification yourself

An extension can also post a notification directly rather than just registering the routing. This requires the [`notifications:write`](permissions.md) permission. If the pane id is empty, Muxy routes to the first pane of the active worktree.
