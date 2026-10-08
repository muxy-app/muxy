# Getting Started

Muxy 2 is in beta. It installs as **Muxy Beta**, next to Muxy 1.x, and keeps its
own settings and data.

## Requirements

| Part | Runs on |
| --- | --- |
| Desktop app | macOS 14 or newer. Beta builds are for Apple Silicon. |
| `muxy` CLI and server on macOS | macOS 14 or newer, on Intel (x86_64) or Apple Silicon (ARM64) |
| `muxy` CLI and server on Linux | glibc 2.35 or newer, on x86_64 or ARM64. musl is unsupported. |

## Install the desktop app

1. Download `Muxy-<version>-arm64.dmg` from the latest `v2.0.0-beta.*` release
   on the [releases page](https://github.com/muxy-app/muxy/releases).
2. Drag **Muxy Beta** to `/Applications` and open it.
3. Optional: choose **Install Command Line Tool…** from the app menu to use
   [`muxy`](../features/muxy-cli.md) in your shell. It links
   `~/.local/bin/muxy`.

The app updates itself. Running terminals keep going through most updates.

## CLI and server only

Install `muxy` and `muxy-server` without the desktop app on macOS or Linux.
Choose a published version from the [releases page](https://github.com/muxy-app/muxy/releases):
`X.Y.Z` for stable or `X.Y.Z-beta.N` for beta, without the leading `v`.
The release must include `install-muxy.sh` and the CLI/server archives.
Muxy 1.x does not include this pair; use beta until a stable Muxy 2 is available.

Set `VERSION` to your chosen release, then run the same commands on either OS:

```bash
VERSION=2.0.0-beta.1123
curl -fsSL "https://github.com/muxy-app/muxy/releases/download/v$VERSION/install-muxy.sh" \
  | sh -s -- --version "$VERSION"
export PATH="$HOME/.local/bin:$PATH"
```

The installer detects your OS and architecture and installs both commands into
`~/.local/bin`. Add the `export` line to your shell's startup file to keep them
on `PATH`. Check both versions with `muxy --version` and `muxy-server --version`,
then run `muxy` for the [terminal UI](../features/terminal-ui.md).

Rerun the installer with `--replace` to update, or add `--install-dir PATH` to
choose another directory. Updates do not restart a running
[server](../features/server.md#lifetime).

Once stable Muxy 2 is available, Homebrew users on either OS can instead run
`brew install muxy-app/tap/muxy-cli`, and update with `brew upgrade muxy-cli`.

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
