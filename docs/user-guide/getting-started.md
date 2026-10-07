# Getting Started

Muxy 2 is in beta. It installs as **Muxy Beta**, next to Muxy 1.x, and keeps its
own settings and data.

## Requirements

| Part | Runs on |
| --- | --- |
| Desktop app | macOS 14 or newer. Beta builds are for Apple Silicon. |
| `muxy` CLI and server only | macOS, or Linux with glibc 2.35 or newer, on x86_64 or ARM64 |

## Install the desktop app

1. Download `Muxy-<version>-arm64.dmg` from the latest `v2.0.0-beta-*` release
   on the [releases page](https://github.com/muxy-app/muxy/releases).
2. Drag **Muxy Beta** to `/Applications` and open it.
3. Optional: choose **Install Command Line Tool…** from the app menu to use
   [`muxy`](../features/muxy-cli.md) in your shell. It links
   `~/.local/bin/muxy`.

The app updates itself. Running terminals keep going through most updates.

## CLI and server only

On a server, or on Linux, install `muxy` and `muxy-server` without the desktop
app. Set `VERSION` to a version from the releases page:

```bash
VERSION=2.0.0-beta-N
curl -fsSL "https://github.com/muxy-app/muxy/releases/download/v$VERSION/install-muxy.sh" \
  | sh -s -- --version "$VERSION"
```

This installs into `~/.local/bin`. Add it to your `PATH` if needed. Run the
same command with `--replace` to update. Then run `muxy` for the
[terminal UI](../features/terminal-ui.md).

## First steps

1. **Add a project.** Choose **Add Project** at the bottom of the sidebar, or
   press `Cmd+O`, and pick a folder.
2. **Open a terminal.** `Cmd+T` opens a tab in the project. `Cmd+N` opens one
   in Home.
3. **Split it.** `Cmd+D` splits right, `Cmd+Shift+D` splits down.
4. **Quit freely.** Terminals keep running in the background. Reopen Muxy and
   everything is where you left it.

## Coming from Muxy 1.x

Open **Settings → Backup & Restore** and choose **Import installed 1.x**. It
brings over supported settings, shortcuts, local projects, workspaces, and
terminal tabs, and lists what it skipped.

Some 1.x features work differently or are not in 2.x yet. Extensions keep
working; see [Coming from 1.x](../extensions/get-started.md#coming-from-1x).

## Next

- [How Muxy works](../features/server.md): the server, sessions, and history.
- [Projects](../features/projects.md), [Terminal](../features/terminal.md), and
  [Keyboard shortcuts](keyboard-shortcuts.md).
- [Remote servers](../features/remote-servers.md) and
  [Mobile](../features/mobile.md).
- [Settings](settings.md) and [Troubleshooting](troubleshooting.md).
