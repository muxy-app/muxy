# Linux desktop

Linux packages target **Ubuntu 24.04 on x86_64 and ARM64**, with glibc 2.39 or
newer and a working Vulkan driver. Linux support is experimental; see
[platform limits and WebKit sandbox setup](../tech/constraints.md#experimental-linux-desktop).
Choose one installation format per channel to avoid conflicting CLI commands.

## Quick installer with in-app updates

Install the Ubuntu dependencies once:

```sh
sudo apt update
sudo apt install curl python3 ca-certificates libfreetype6 libfontconfig1 libxcb1 \
  libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 libvulkan1 mesa-vulkan-drivers \
  libwebkit2gtk-4.1-0 xdg-utils xdg-desktop-portal bubblewrap xdg-dbus-proxy
```

Then install the **latest stable release**. The installer detects your
architecture and verifies the download. Run it as your normal user, without `sudo`:

```sh
curl -fsSL --proto '=https' --proto-redir '=https' \
  https://github.com/muxy-app/muxy/releases/latest/download/install-desktop-linux.py \
  | python3 - install
```

Open **Muxy** from Applications. By default, the app, CLI and server install into
`~/.local/share/muxy-desktop`. Add `~/.local/bin` to your shell's `PATH` to use
`muxy <folder>` from a terminal.

## Debian package

Download `muxy-desktop_<version>_amd64.deb` or
`muxy-desktop_<version>_arm64.deb` from the
[latest stable release](https://github.com/muxy-app/muxy/releases/latest).
Use `dpkg --print-architecture` to check which you need. In your download directory:

```sh
sudo apt install './muxy-desktop_<version>_amd64.deb'
```

Replace the filename with your download. APT installs the dependencies and adds
**Muxy** to Applications, with `muxy` and `muxy-server` in `/usr/bin`.

## AppImage

Install the [runtime libraries above](#quick-installer-with-in-app-updates), then
download `Muxy-<version>-linux-x86_64.AppImage` or
`Muxy-<version>-linux-arm64.AppImage` from the
[latest stable release](https://github.com/muxy-app/muxy/releases/latest).
Use `uname -m` to check: `aarch64` means ARM64. Replace the filename below with
your download and run it without `sudo`:

```sh
chmod +x './Muxy-<version>-linux-x86_64.AppImage'
'./Muxy-<version>-linux-x86_64.AppImage'
```

The first launch adds **Muxy** to Applications with its app icon. AppImage uses
**system WebKitGTK**, not a bundled browser engine. If FUSE is unavailable,
append `--appimage-extract-and-run` when launching it.

Muxy keeps an unpacked copy in
`${XDG_DATA_HOME:-$HOME/.local/share}/muxy-appimage/<bundle-id>` so terminals can
keep running after the window closes. This takes extra disk space. Use
`./Muxy-<version>-linux-x86_64.AppImage --cli <arguments>` for the bundled CLI,
or choose **Install Command Line Tool…** in the app to link it into `~/.local/bin`.

## Beta

To install the latest beta, append `--channel beta` to the quick-installer
command. For `.deb` or AppImage, choose a beta from the
[releases page](https://github.com/muxy-app/muxy/releases).
Beta installs as **Muxy Beta** alongside stable, with separate settings and sessions.
The quick installer and Debian package provide `muxy-beta` and `muxy-server-beta` commands.

## Updates

- **Quick installer:** the app checks automatically; **Check for Updates** checks
  now. Stable installs stay on stable; beta installs stay on beta.
- **Debian package:** download the newer `.deb` and install it with `sudo apt install`.
  Stop the server and quit Muxy first. There is no Muxy APT repository, so
  `apt upgrade` alone does not update Muxy.
- **AppImage:** quit the app, then download and run the newer file. Running
  terminals stay alive. If you linked the CLI, choose **Install Command Line Tool…**
  again from the new app.

Debian packages and AppImages do not use the in-app updater.

## Remove

Stop the local server in **Settings → Server**, then quit Muxy. Stopping the
server ends its running terminals. Removal keeps your settings, projects and
saved session data.

- **Quick installer:** run the command below. For beta, replace `muxy-desktop`
  with `muxy-desktop-beta`.

  ```sh
  python3 "${XDG_DATA_HOME:-$HOME/.local/share}/muxy-desktop/current/install.py" uninstall
  ```

- **Debian package:** run `sudo apt remove muxy-desktop` (or `muxy-desktop-beta`).
- **AppImage:** delete the downloaded file, its matching unpacked copy listed
  above, and any CLI link or Applications entry pointing into that copy. The entry
  is in `${XDG_DATA_HOME:-$HOME/.local/share}/applications/com.muxy.app.desktop`
  (`com.muxy-beta.app.desktop` for beta). Do not delete a copy still used by
  another running app or server.
