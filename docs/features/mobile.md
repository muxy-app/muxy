# Mobile

The Muxy phone apps connect straight to the server on your computer. There is
no cloud service and no account. A paired phone sees the same projects and
terminals as your desktop.

A phone connects in one of two ways:

- **Paired:** over your local network, a VPN such as Tailscale, or a public
  address. The connection is encrypted and the phone trusts only your server.
- **Over SSH:** the phone logs in like `muxy --host` does. No pairing or open
  port needed. See [Remote servers](remote-servers.md).

## Pair a phone

From the desktop app:

1. Open the **Devices** tab with **Manage Devices…** in the sidebar's
   **Devices** popover. Under **This Computer**, turn on **Allow other
   devices**.
2. Under **Pair a device**, choose **Show Pairing Code**.
3. Scan the code with the Muxy app.

From a shell, on any computer with `muxy`:

```bash
muxy mobile enable
muxy mobile pair
```

- A pairing code works once, for five minutes, and only while it is on screen.
- The code lists your computer's addresses. If phones reach it by another name,
  such as a cloud server's public name, put it first:
  `muxy mobile pair --address devbox.example.com`.
- Phones connect on port `7419` by default. Change it in the **Devices** tab
  or with `muxy mobile enable --port <port>`.

## Manage phones

| Task | Desktop | CLI |
| --- | --- | --- |
| See status and phones | The **Devices** tab | `muxy mobile` |
| Revoke a phone | **Revoke** next to it | `muxy mobile revoke <id>` |
| Turn access off | **Allow other devices** off | `muxy mobile disable` |

Revoking a phone, or turning access off, disconnects it at once.

## What a phone can do

A paired phone gets full terminal access: it can use projects, terminals, Git,
and files. It can't manage device access, change server settings, stop the
server, or run programs with `exec`.

Other computers pair with the same code's link; see
[Remote servers](remote-servers.md#pair-with-another-computer).

## Troubleshooting

- On macOS, allow `muxy-server` if the firewall asks.
- On iOS, the Muxy app needs local network permission.
- The phone and computer must reach each other: same network, a VPN, or an
  address added with `--address`.
