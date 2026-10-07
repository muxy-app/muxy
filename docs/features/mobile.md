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

1. Open **Settings → Mobile** and turn on **Allow mobile devices**.
2. Under **Pair a phone**, choose **Show Pairing Code**.
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
- Phones connect on port `7419` by default. Change it in **Settings → Mobile**
  or with `muxy mobile enable --port <port>`.

## Manage phones

| Task | Desktop | CLI |
| --- | --- | --- |
| See status and phones | **Settings → Mobile** | `muxy mobile` |
| Revoke a phone | **Revoke** next to it | `muxy mobile revoke <id>` |
| Turn access off | **Allow mobile devices** off | `muxy mobile disable` |

Revoking a phone, or turning access off, disconnects it at once.

## What a phone can do

A paired phone gets full terminal access: it can use projects, terminals, Git,
and files. It can't manage mobile access, change server settings, stop the
server, or run programs with `exec`.

## Troubleshooting

- On macOS, allow `muxy-server` if the firewall asks.
- On iOS, the Muxy app needs local network permission.
- The phone and computer must reach each other: same network, a VPN, or an
  address added with `--address`.
