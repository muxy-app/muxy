# Remote Server Overview

Muxy embeds a WebSocket server that lets external clients connect to the desktop app over the local network — for mobile companions, dashboards, and custom integrations.

```mermaid
flowchart TB
  Client[Mobile / dashboard client]
  Client <-->|WebSocket / JSON| Muxy[Muxy.app]
  Muxy --> Settings[Settings → Mobile]
  Muxy --> Approved[approved-devices.json]
  Muxy -.->|Bonjour _muxy._tcp| Client
```

## Pages

| Page | What's in it |
| --- | --- |
| [Setup](setup.md) | Enable the server, port, discovery, security model, error codes |
| [Pairing](pairing.md) | Authenticate, pair, register flow |
| [Protocol](protocol.md) | Message envelope, request/response/event |
| [Methods](methods.md) | Every RPC method, its parameters, and result shapes |
| [Events](events.md) | Server-pushed events and their payloads |
| [Data Objects](data-objects.md) | Project, Worktree, Workspace, Notification, terminal cells, logo |

## Quick reference

- Endpoint: `ws://<host>:<port>` (default port `4865`; `4866` in development builds)
- Format: JSON, UTF-8, ISO-8601 dates, UUID strings, RGB colors as `0xRRGGBB` integers
- Disabled by default; enable in **Settings → Mobile**
- All clients must authenticate before any other RPC is accepted
- The server advertises over Bonjour as `_muxy._tcp`

## Recommended client startup

```mermaid
flowchart TB
  Connect[Connect WebSocket] --> Auth[authenticateDevice]
  Auth -->|401 unknown device| Pair[pairDevice]
  Pair -->|approved| Ready
  Pair -->|403 denied| Stop[Show error]
  Auth -->|pairing ok| Ready
  Ready[clientID issued] --> List[listProjects]
  List --> Pick[selectProject]
  Pick --> Wt[listWorktrees + selectWorktree]
  Wt --> Ws[getWorkspace]
  Ws --> Subscribe[Optional: load logos / VCS state / takeOverPane]
```
