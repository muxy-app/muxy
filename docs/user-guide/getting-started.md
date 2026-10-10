# Getting Started

## Requirements

| Part | Runs on |
| --- | --- |
| Desktop app | macOS 14 or newer, on Apple Silicon or Intel |
| `muxy` CLI and server | macOS 14 or newer, or Linux with glibc 2.35 or newer, on x86_64 or ARM64. musl is unsupported. |

## Desktop app

Install with [Homebrew](https://brew.sh):

```bash
brew install --cask muxy-app/tap/muxy
```

Or download it yourself:

1. From the [latest release](https://github.com/muxy-app/muxy/releases/latest),
   download `Muxy-<version>-arm64.dmg` for Apple Silicon or
   `Muxy-<version>-x86_64.dmg` for Intel.
2. Drag **Muxy** to `/Applications` and open it.

To use [`muxy`](../features/muxy-cli.md) in your shell, choose **Install Command
Line Tool…** from the app menu. It links `~/.local/bin/muxy`.

The app updates itself, also when installed with Homebrew. Running terminals
keep going through most updates.

## CLI and server only

For Linux, or a Mac without the desktop app. This installs `muxy` and
`muxy-server`.

### Homebrew

```bash
brew install muxy-app/tap/muxy-cli
```

Update with `brew upgrade muxy-cli`. On a computer you will reach as a
[remote server](../features/remote-servers.md), use the install script instead:
SSH sessions often don't have Homebrew on `PATH`.

### Install script

Set `VERSION` to the newest version on the
[releases page](https://github.com/muxy-app/muxy/releases/latest), without the
leading `v`:

```bash
VERSION=2.1.0
curl -fsSL "https://github.com/muxy-app/muxy/releases/download/v$VERSION/install-muxy.sh" \
  | sh -s -- --version "$VERSION"
export PATH="$HOME/.local/bin:$PATH"
```

The script picks the build for your OS and architecture, verifies its checksum,
and installs both commands into `~/.local/bin`. Add the `export` line to your
shell's startup file to keep them on `PATH`.

- **Update:** run it again with the new version and `--replace`.
- **Another folder:** add `--install-dir PATH`.

### Check it

Run `muxy --version` and `muxy-server --version`, then `muxy` to open the
[terminal UI](../features/terminal-ui.md). Updates never restart a running
[server](../features/server.md#lifetime).

## Upgrading from Muxy 1.x

Muxy 1.x offers Muxy 2 as an update, or you can install Muxy 2 over it. Unless
you already have projects in Muxy 2, the first launch imports your 1.x setup:
supported settings, shortcuts, local projects, workspaces, terminal tabs, the
default shell, and extensions. Anything left out is listed, and your 1.x data
stays where it was. To import again later, open **Settings → Backup & Restore**
and choose **Import installed 1.x**; it adds to what you already have.

Some 1.x features work differently or are not in 2.x yet. Extensions keep
working; see [Coming from 1.x](../extensions/get-started.md#coming-from-1x).

## Beta

The beta gets new features first. It installs as a separate app, **Muxy Beta**,
next to Muxy, keeps its own settings and data, and updates to each new beta.
Download it from the newest `v<version>-beta.<N>` pre-release on the
[releases page](https://github.com/muxy-app/muxy/releases). For the CLI and
server, give the install script a beta version, such as `2.1.0-beta.1143`.

## First steps

1. **Add a project.** Choose **Add Project** at the bottom of the sidebar, or
   press `Cmd+O`, and pick a folder.
2. **Open a terminal.** `Cmd+T` opens a tab in the project. `Cmd+N` opens one
   in Home.
3. **Split it.** `Cmd+D` splits right, `Cmd+Shift+D` splits down.
4. **Quit freely.** Terminals keep running in the background. Reopen Muxy and
   everything is where you left it.

## Next

- [How Muxy works](../features/server.md): the server, sessions, and history.
- [Projects](../features/projects.md), [Terminal](../features/terminal.md), and
  [Keyboard shortcuts](keyboard-shortcuts.md).
- [Remote servers](../features/remote-servers.md) and
  [Mobile](../features/mobile.md).
- [Settings](settings.md) and [Troubleshooting](troubleshooting.md).
