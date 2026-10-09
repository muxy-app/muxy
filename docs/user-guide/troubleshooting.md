# Troubleshooting

Common problems and fixes. If yours isn't here, please
[open an issue](https://github.com/muxy-app/muxy/issues).

## Logs

| What | Where |
| --- | --- |
| Server | `server.log` in the [profile folder](../features/server.md#files) |
| Desktop app | Start it with `MUXY_DIAGNOSTICS=1` to write `app-diagnostics-<pid>.log` to the profile folder |
| Extensions | **Settings → Extensions →** the extension **→ Reveal log** |
| CLI and terminal UI | Printed in the terminal |

`muxy --build-info` and `muxy server status` show the versions in use.

## The app doesn't open

Usually `settings.toml` has an error or an unknown key. Start the app from a
terminal to see why:

```bash
"/Applications/Muxy Beta.app/Contents/MacOS/muxy-app"
```

Fix the line it names, or move `settings.toml` out of the profile folder to
start with defaults.

## The server won't start

If `muxy` reports `request timed out`, run the server in the foreground to see
the error, often a typo in `server.toml`:

```bash
"/Applications/Muxy Beta.app/Contents/MacOS/muxy-server"   # or ~/.local/bin/muxy-server
```

## `muxy` is not found, or is the wrong version

The CLI installs into `~/.local/bin`. Add it to your `PATH`:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

If `muxy --version` shows 1.x, Muxy 1.x's CLI comes first on your `PATH`.

## A remote server won't connect

- **Host key not trusted:** run `ssh <host>` once and accept the key.
- **Login refused:** check that `ssh <host>` works with your key or agent. Only
  the desktop app supports passwords.
- **Muxy isn't installed:** install it from the **Remote** section, or run the
  installer there. See [Remote servers](../features/remote-servers.md).
- **Incompatible version:** install the same version on both computers. If the
  version is right but the old server is still running, run
  `pkill -x muxy-server` there. This ends its terminals.
- **Terminals end when SSH disconnects (Linux):** run `loginctl enable-linger`.

## A phone can't connect

- Check **Settings → Mobile → Status** says it is listening.
- On macOS, allow `muxy-server` if the firewall asks.
- On iOS, allow the Muxy app to use the local network.
- The phone must reach the computer: same network, a VPN, or an address added
  with `muxy mobile pair --address`.

## No agent notifications

- Allow notifications in **System Settings → Notifications → Muxy Beta**.
- Muxy doesn't notify for the pane you're looking at, or for detached
  terminals.
- Recognition is best effort. `muxy activity list` shows what the server sees.

## Git or AI actions are disabled

- Install and sign in to a supported [AI tool](../features/git.md#ai-tools) in
  a terminal, then try again.
- Pull request features need `gh` installed and signed in on the project's
  server: `gh auth login`.
- AI actions run on this Mac. For a remote project, the same folder must exist
  here.
- Actions are disabled on a clean tree, a detached HEAD, during conflicts, or
  while another action runs.

## Keys don't work as expected

- `Option` acts as `Alt` in terminals. Set `macos-option-as-alt = false` in
  `ghostty.conf` to type special characters.
- A `keybind` in `ghostty.conf` wins over app shortcuts while a terminal is
  focused.

## Start fresh

1. Stop the server, which ends every terminal: `muxy server stop --force`.
2. Quit Muxy.
3. Move the [profile folder](../features/server.md#files) somewhere else.

Muxy 1.x data and `~/.config/ghostty` are not touched. The next launch imports
Muxy 1.x again; to start empty, also move `~/Library/Application Support/Muxy`.

## Reporting a bug

Include your macOS or Linux version, the output of `muxy --build-info`, steps
to reproduce, and the relevant lines of `server.log`.
