# Extensions

> **Status:** under active development. Marked **DEV** in **Settings → Extensions**. The manifest format, permission set, and wire format may change without notice.

User-installed subprocesses that Muxy launches and talks to over the existing notification Unix socket. Extensions can react to workspace events, register palette commands, post notifications, and (with permission) drive the same verbs the `muxy` CLI exposes.

## Pages

| Page | What's in it |
| --- | --- |
| [Overview](overview.md) | Architecture, lifecycle, security model |
| [Manifest](manifest.md) | `manifest.json` fields, validation, subprocess environment |
| [Permissions](permissions.md) | What each permission grants, what isn't gated |
| [Events](events.md) | Identify/subscribe handshake, event list, wire format |
| [Palette Commands](palette-commands.md) | Register commands and react to triggers |
| [AI Provider Hooks](ai-provider.md) | Route third-party agent notifications to a custom source |

## Quick reference

- Install path: `~/.config/muxy/extensions/<name>/`
- Transport: `~/Library/Application Support/Muxy/muxy.sock` (same socket as `muxy` CLI)
- Subprocess environment: `MUXY_SOCKET_PATH`, `MUXY_EXTENSION_ID`
- Sticky verbs: `identify|<id>`, `subscribe|<event>`
- See [the muxy CLI feature page](../features/muxy-cli.md) for the verb vocabulary

## Minimal example

```
~/.config/muxy/extensions/hello/
  manifest.json
  run.sh
```

```json
{
  "name": "hello",
  "version": "0.1.0",
  "entrypoint": "run.sh",
  "permissions": ["notifications:write"],
  "events": ["pane.created"],
  "commands": [
    { "id": "ping", "title": "Hello: Ping" }
  ]
}
```

```bash
#!/bin/bash
{
  printf 'identify|%s\n' "$MUXY_EXTENSION_ID"
  printf 'subscribe|pane.created\n'
  printf 'subscribe|command.ping\n'
  while sleep 60; do :; done
} | nc -U "$MUXY_SOCKET_PATH" | while IFS= read -r line; do
  echo "$line" >&2
done
```
