# Remote Server (MuxyServer)

The desktop app embeds a WebSocket server (`MuxyRemoteServer`) that exposes workspace state and terminal operations to remote clients (e.g. the MuxyMobile companion app) over the local network. The user-facing wire protocol is documented in [Remote Server feature docs](../../features/remote-server/README.md).

## Architecture

```mermaid
flowchart TB
  Client[(Remote client)] <-->|WebSocket / JSON| Server[MuxyRemoteServer]
  Server --> Delegate[MuxyRemoteServerDelegate]
  Delegate --> AppState
  Delegate --> ProjectStore
  Delegate --> Approved[ApprovedDevicesStore]
  Delegate --> Pairing[PairingRequestCoordinator]
  Server --> Lifecycle[MobileServerService]
  Lifecycle --> Settings[Mobile settings UI]
```

The server uses Apple's Network framework (`NWListener` + `NWConnection`) with the WebSocket protocol. All messages use the `MuxyMessage` JSON envelope from `MuxyShared`. The user-configurable port (default 4865) is stored in `UserDefaults` and applied on start. `MobileServerService` reports bind failures back to the UI: if the listener fails (e.g. port in use), the enable toggle is rolled off and the settings view displays the error.

## Pairing handshake

```mermaid
sequenceDiagram
  participant C as Client
  participant S as MuxyRemoteServer
  participant A as ApprovedDevicesStore
  participant P as PairingRequestCoordinator
  participant U as User on Mac

  C->>S: authenticateDevice(deviceID, token)
  S->>A: lookup token hash
  alt known + match
    A-->>S: ok
    S-->>C: pairing { clientID }
  else unknown / mismatch
    A-->>S: 401
    S-->>C: 401
    C->>S: pairDevice(deviceID, token)
    S->>P: enqueue request
    P->>U: approval sheet
    alt approve
      U-->>P: approve
      P->>A: store SHA-256 token hash
      P-->>S: ok
      S-->>C: pairing { clientID }
    else deny
      U-->>P: deny
      P-->>S: 403
      S-->>C: 403
    end
  end
```

After success, the client is added to an `authenticatedClients` set on `MuxyRemoteServer`; broadcasts only go to clients in that set. Until the handshake succeeds the server rejects every other RPC with `401`. The `Mobile` tab in Settings lists approved devices with a Revoke action; revoke removes the device from `approved-devices.json` and terminates active connections via `MuxyRemoteServer.disconnect(deviceID:)`.

A token mismatch is treated the same as unknown (returns `401`) so a stolen but outdated credential can't resume authentication.

## Terminal I/O streaming

Terminal traffic flows as raw PTY bytes, not rendered cells. This relies on two additive exports on the `muxy-app/ghostty` fork (see [Building Ghostty](../building-ghostty.md)):

- `ghostty_surface_set_data_callback(surface, cb, userdata)` — per-surface callback invoked on the termio thread on every chunk of bytes from the PTY, before Ghostty's emulator parses them.
- `ghostty_surface_send_input_raw(surface, ptr, len)` — writes bytes directly to the PTY, bypassing the paste pipeline (no bracketed-paste wrapping, no newline filtering, no keyboard-protocol interpretation).

```mermaid
sequenceDiagram
  participant Shell as Shell / TUI
  participant PTY
  participant GH as Ghostty
  participant Streamer as RemoteTerminalStreamer
  participant Server as MuxyRemoteServer
  participant Client as Remote client

  Shell->>PTY: write
  PTY-->>GH: bytes (termio thread)
  GH-->>Streamer: data callback
  Streamer->>Server: terminalOutput { paneID, base64 bytes }
  Server-->>Client: event
  Client->>Server: terminalInput { paneID, base64 bytes }
  Server->>GH: ghostty_surface_send_input_raw
  GH->>PTY: write
```

`RemoteTerminalStreamer` registers the data callback on every terminal surface at creation (`GhosttyTerminalNSView.createSurface`), unregisters on teardown, and forwards bytes as `terminalOutput` events targeted at the owning client via `MuxyRemoteServer.send(_:to:)`. The event payload is a `TerminalOutputEventDTO` containing the paneID and a `Data` of raw bytes (base64-encoded on the JSON wire).
