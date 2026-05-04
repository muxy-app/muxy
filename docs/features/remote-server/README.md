# Remote Server API

Muxy embeds a WebSocket server that lets external clients connect to the desktop app over the local network — for mobile companions, dashboards, and custom integrations.

```mermaid
flowchart TB
  Client[Mobile / dashboard client]
  Client <-->|WebSocket / JSON| Muxy[Muxy.app]
  Muxy --> Settings[Mobile settings]
  Muxy --> Approved[Approved devices]
```

## Pages

| Page | What's in it |
| --- | --- |
| [Setup](setup.md) | Enable the server, port, security model |
| [Pairing](pairing.md) | Authenticate, pair, register flow |
| [Protocol](protocol.md) | Message envelope, request/response/event |
| [Methods](methods.md) | Every RPC method and its parameters |
| [Events](events.md) | Server-pushed events and their payloads |
| [Data Objects](data-objects.md) | Project, Worktree, Workspace, Notification, Terminal snapshot |

## Quick reference

- Endpoint: `ws://<host>:<port>` (default port `4865`)
- Format: JSON, UTF-8, ISO-8601 dates, UUID strings
- Disabled by default; enable in **Settings → Mobile**
- All clients must authenticate before any other RPC is accepted

## Recommended client startup

```mermaid
flowchart TB
  Connect[Connect WebSocket] --> Auth[authenticateDevice]
  Auth -->|401| Pair[pairDevice]
  Pair -->|approved| Ready
  Auth -->|ok| Ready
  Ready[clientID issued] --> List[listProjects]
  List --> Pick[selectProject]
  Pick --> Wt[listWorktrees + selectWorktree]
  Wt --> Ws[getWorkspace]
  Ws --> Subscribe[Optional: subscribe / load logos / VCS state]
```
