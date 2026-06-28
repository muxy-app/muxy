# Setup & Security

## Enabling the server

The Mobile server is **disabled by default in release builds**. Development builds start enabled on port `4866` so a debug build can run alongside a release build. Toggle it from **Settings -> Mobile** on macOS.

| Setting | Default | Notes |
| --- | --- | --- |
| Allow mobile device connections | off in release, on in development | Starts/stops the WebSocket listener. |
| Port | `4865` in release, `4866` in development | Stored in `UserDefaults`. Changing it stops the server and turns the toggle off; re-enable it to start on the new port. A bind failure retires the listener and surfaces the error; the setting remains enabled until the user turns it off or fixes the port. |
| Approved devices | empty | List of paired clients, each with a **Revoke** button. |

The valid port range is `1024`–`65535`. If the port is already in use, the settings panel shows a **Free Port** action that terminates the process currently listening on it.

## Endpoint

- Protocol: WebSocket (text frames; binary frames are also accepted)
- URL: `ws://<host>:<port>`
- Encoding: UTF-8 JSON
- Date format: ISO 8601
- IDs: UUID strings

## Discovery & pairing link

When the server is enabled, the Mac advertises itself over Bonjour as `_muxy._tcp` and the settings panel shows a QR code and a pairing URI:

```
muxy://pair?host=<host>.local&port=<port>&service=<name>&label=<name>
```

The QR/URI carry the host, port, and a friendly label — **never the token**. First-time pairing still requires explicit approval on the Mac. When a Tailscale interface is present, a second pairing host (the `100.64.0.0/10` Tailscale IP) is offered alongside the `.local` hostname.

## Security model

The API is designed for trusted local networks.

- Transport is `ws://`, not TLS.
- Clients must authenticate before any other RPC.
- New devices must be approved on the Mac before they become trusted.
- Tokens are compared in constant time and only their SHA-256 hash is stored on disk.

For production integrations, treat the connection as local-network only unless you provide your own secure tunnel such as Tailscale or a VPN.

## Error codes

| Code | Constant | Meaning |
| --- | --- | --- |
| `400` | invalidParams | Invalid or mismatched parameters |
| `401` | unauthorized | Authentication required, or unknown device |
| `403` | pairingDenied / forbidden | Pairing denied, wrong token, or consent denied |
| `404` | notFound | Resource not found |
| `408` | pairingTimeout | Reserved; current builds do not emit a pairing timeout |
| `500` | internalError | Internal error or operation failure |
| `502` | — | Extension handler threw or is unregistered |
| `503` | extensionUnavailable | Extension not running |
| `504` | timeout | Extension handler timed out |

`502`–`504` only originate from `extensionRequest`.

## Integration recommendations

- Persist `deviceID` and `token` securely.
- Re-authenticate after reconnecting; the `clientID` is per-connection and changes.
- Treat `workspaceChanged` as authoritative for layout and tab state.
- Cache project logos after decoding the Base64 payload.
- Call `takeOverPane` before any interactive terminal control; input from a non-owner is dropped.
- Handle a `401` on `authenticateDevice` by falling back to `pairDevice`; a `403` means a wrong token — re-pair.
- Do not rely on `subscribe`/`unsubscribe` for filtering — every event reaches every authenticated client.
