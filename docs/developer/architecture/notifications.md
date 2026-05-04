# Notification System

Notifications alert users when terminal events occur (command completion, AI agent messages, OSC escape sequences). Each carries full navigation context so click-to-focus jumps to the originating pane.

## Sources & flow

```mermaid
flowchart TB
  OSC[OSC 9/777<br/>terminal escape] --> Adapter[GhosttyRuntimeEventAdapter]
  Claude[Claude Code hooks] --> Sock[Unix socket<br/>muxy.sock]
  External[External tools] --> Sock
  Sock --> Server[NotificationSocketServer]
  Adapter --> Lookup[TerminalViewRegistry<br/>paneID lookup]
  Server --> Lookup
  Lookup --> Nav[NotificationNavigator<br/>resolveContext]
  Nav --> Store[NotificationStore.add]
  Store -->|focused + active| Drop[suppress]
  Store --> Toast[Toast + sound]
  Store --> Persist[notifications.json<br/>debounced]
  Store --> UI[badge + panel]
```

| Source | Mechanism |
| --- | --- |
| OSC 9 / 777 | `GHOSTTY_ACTION_DESKTOP_NOTIFICATION` in `GhosttyRuntimeEventAdapter`. |
| Claude Code | Wrapper script injects `--hooks` to route lifecycle events through the socket. |
| Unix socket | `~/Library/Application Support/Muxy/muxy.sock`, pipe-delimited messages with paneID. |

## Click-to-navigate

`NotificationNavigator.navigate(to:)` dispatches `selectProject` → `focusArea` → `selectTab` against `AppState`. System notifications encode the navigation context in `userInfo` and bring the app to front on click.

## Pane environment

Every terminal surface receives `MUXY_PANE_ID`, `MUXY_PROJECT_ID`, `MUXY_WORKTREE_ID`, and `MUXY_SOCKET_PATH` via `ghostty_surface_config_s.env_vars`. The Claude wrapper script and any external sender uses these to identify the originating pane.
