# Templates

E2E runs clone these SwiftVM templates. Their names are set in
`scripts/e2e` (`template_for`); keep that the only place. The names are
stable: a refresh replaces a template under the same name.

| Platform | Template | Guest |
| --- | --- | --- |
| `macos` | `muxy-macos` | macOS 27.2 ARM64, Xcode 27.2, Homebrew Zig 0.17.0 |
| `wayland` | `muxy-ubuntu-wayland` | Ubuntu 24.04.5 ARM64, GNOME 46 native Wayland, Zig 0.17.0 |
| `x11` | `muxy-ubuntu-x11` | Ubuntu 24.04.5 ARM64, GNOME 46 native X11, Zig 0.17.0 |
| `windows` | `muxy-windows-11` | Windows 11 Pro ARM64 26300, VS 2022 Build Tools, LLVM, PowerShell 7, Zig 0.17.0 |

Each holds:

- the `muxy` test account logged in to its desktop, with no screen saver,
  screen lock or screen blanking
- Agent Desktop ready (`swiftvm desktop <vm> status`). On macOS that means
  Accessibility and Screen Recording are granted. On Wayland it means the
  portal's screenshot permission is granted (`prepare-desktop.sh`), because
  its consent dialog takes no synthetic input.
- the toolchains, the source and a warm build cache in `~/src/muxy` and
  `~/.cache/muxy-vm/target`
- `scripts/guest/muxy-vm-build` and `muxy-vm-run` in `~/.local/bin`, and the
  sandbox marker `~/.local/share/muxy-vm/managed`
- clipboard tools (`pbcopy`, `wl-clipboard`, `xclip`), a Git identity and a
  `~/.zshrc`. Windows has the same folders under `C:\Users\muxy`, with the
  `.ps1` helpers in `~\.local\bin`.
- no Muxy test profile

On macOS, `swiftvm-agent` (which launches the app, so macOS charges the app's
file access to it) is allowed into Desktop, Documents, Downloads and Music.
Without that, the project picker raises a consent dialog over the app. A new
macOS template needs the same grants: in a Muxy terminal launched with
`muxy-vm-run`, run `ls ~/Desktop`, allow the prompt, and repeat for each
folder, one at a time.

Never share the host's home, SSH agent, credentials, Muxy profiles or a
writable checkout with a guest. `scripts/export-source.py` copies the working
tree without Git metadata, ignored files or secrets.

## When to refresh

- `e2e up` builds for more than ~5 minutes on a warm template (stale cache:
  the Rust toolchain, Zig, Ghostty or many dependencies changed).
- A test needs a tool the guest lacks, or a fresh clone needs any setup.
- `e2e up` stops with `ENV: Agent Desktop ...`.
- On macOS, notification banners ("Multiple Extensions Added", "App
  Background Activity") cover the title bar after boot. `e2e banners`
  works around it; a refresh should dismiss them before saving.
- The OS or Xcode needs to change (that is also a test-matrix change; ask).

## How to refresh

Never modify a template's own machine or a test VM. Commands here run from
the repository root with `S=.agents/skills/e2e-test/scripts`. For each
platform:

1. `swiftvm create --template <name> --name muxy-maint-<platform>` and wait
   for its desktop: `swiftvm desktop <vm> status` answers.
2. Add any missing setup to the scripts in `$S/setup` first, so a rebuild
   from nothing gets it too, then run it in the guest
   (`swiftvm exec <vm> -- bash -s < "$S/setup/<script>"`).
3. Import the current source and build, as in "Source and build" below.
4. Launch the app once and check a terminal works.
5. Clean, as in "Clean and save" below.
6. `swiftvm save-template muxy-maint-<platform> <name>-new`.
7. Acceptance: point `template_for` at `<name>-new`, then run `e2e prepare`,
   `e2e up` and `e2e g <vm> reset` on that clone, take an `e2e shot`, and
   point `template_for` back. `e2e g <vm> reset` must launch the app.
8. Only then `swiftvm delete-template <name>`,
   `swiftvm rename-template <name>-new <name>`, and
   `swiftvm delete muxy-maint-<platform>`. Update the table above if the
   guest changed.

## Rebuild from nothing

Use this only when a template is gone. The last full rebuild was on
2026-11-01 with SwiftVM guest tools 1.4–1.5, Rust 1.92.0 and Zig 0.17.0.
Recheck `swiftvm --help`, `rust-toolchain.toml` and the Linux CI packages
first. Changing the OS changes the test matrix: ask first.

Each baseline gets 8 CPUs and 16 GB RAM. Linux gets a 128 GB disk and macOS
160 GB, which can't grow later. Check `df -h ~` for 70 GB free (macOS takes
about 44 GB, each Linux machine about 6 GB).

Keep the guest password in `target/swiftvm/credentials.json` (mode 600),
never in logs or commands you print. Pass it from that file through a
subprocess argument:

```sh
mkdir -p target/swiftvm && chmod 700 target/swiftvm
python3 - <<'PY'
import json, os, secrets
from pathlib import Path
p = Path('target/swiftvm/credentials.json')
if not p.exists():
    fd = os.open(p, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, 'w') as f:
        json.dump({'user': 'muxy', 'password': secrets.token_urlsafe(24)}, f)
PY
```

### Wayland

Create it compressed. `create --template` can't add compression later.

```sh
python3 - <<'PY'
import json, subprocess
from pathlib import Path
c = json.loads(Path('target/swiftvm/credentials.json').read_text())
subprocess.run(['swiftvm', 'create', 'ubuntu-24.04', '--name', 'muxy-linux-wayland',
    '--cpus', '8', '--memory', '16', '--disk', '128', '--user', c['user'],
    '--password', c['password'], '--compressed', '--script', 'gnome'], check=True)
PY
```

`create` returning doesn't mean GNOME is installed: wait until `swiftvm info`
shows `Setup: GNOME: finished` (log: `/var/log/swiftvm-setup.log`, through
`exec --root`). If Firefox's Snap download stalls, `snap abort <id>` lets the
setup finish without it. GNOME reboots once. Then:

```sh
VM=muxy-linux-wayland
swiftvm wait "$VM" --timeout 120
swiftvm exec "$VM" --root --timeout 1800 -- bash -s < "$S/setup/setup-linux.sh"
swiftvm exec "$VM" --timeout 1800 -- bash -s < "$S/setup/setup-toolchains.sh"
swiftvm exec "$VM" --root -- bash -s wayland < "$S/setup/configure-linux-session.sh"
swiftvm stop "$VM"
swiftvm share-clipboard "$VM" off
swiftvm start "$VM" && swiftvm wait "$VM" --timeout 120
swiftvm exec "$VM" -- bash -s < "$S/setup/prepare-desktop.sh"
swiftvm desktop "$VM" status
swiftvm desktop "$VM" screenshot /tmp/check.png
```

`loginctl show-session <id> -p Type` must report `wayland` for `muxy`. The
renderer is Mesa's software Vulkan (llvmpipe); SwiftVM doesn't accelerate
Linux graphics.

### X11

Clone the stopped, verified Wayland baseline so its tools and cache are reused:

```sh
swiftvm clone muxy-linux-wayland muxy-linux-x11
swiftvm start muxy-linux-x11 && swiftvm wait muxy-linux-x11 --timeout 120
swiftvm exec muxy-linux-x11 --root -- bash -s x11 < "$S/setup/configure-linux-session.sh"
swiftvm restart muxy-linux-x11 && swiftvm wait muxy-linux-x11 --timeout 120
```

The session must be `Type=x11` (native X11, not XWayland), with `DISPLAY` set
and `WAYLAND_DISPLAY` unset.

### macOS

```sh
python3 - <<'PY'
import json, subprocess
from pathlib import Path
c = json.loads(Path('target/swiftvm/credentials.json').read_text())
subprocess.run(['swiftvm', 'create', 'macos', '--name', 'muxy-macos', '--cpus', '8',
    '--memory', '16', '--disk', '160', '--user', c['user'], '--password', c['password']], check=True)
PY
swiftvm wait muxy-macos --timeout 300
swiftvm share-clipboard muxy-macos off
swiftvm desktop muxy-macos status
```

With `--password`, SwiftVM grants `swiftvm-agent` Accessibility and Screen
Recording. `status` must show both. If one is missing, run
`swiftvm install-tools muxy-macos --password …` (or `allow-accessibility`) and
leave the guest's mouse and keyboard alone while it works. Never edit the TCC
database. If the first boot fails with `Failed to lock auxiliary storage`,
`swiftvm start` again once.

GPUI needs full Xcode and its Metal toolchain. Copy the host's Xcode in, then
install Metal as `muxy`, not root:

```sh
COPYFILE_DISABLE=1 tar -cf target/swiftvm/Xcode.tar -C /Applications Xcode.app
swiftvm cp --root target/swiftvm/Xcode.tar muxy-macos:/tmp/Xcode.tar
swiftvm exec muxy-macos --root --timeout 1800 -- bash -c 'tar -xf /tmp/Xcode.tar -C /Applications && rm /tmp/Xcode.tar && xcode-select -s /Applications/Xcode.app/Contents/Developer && xcodebuild -license accept && xcodebuild -runFirstLaunch'
swiftvm exec muxy-macos --timeout 1800 -- xcodebuild -downloadComponent MetalToolchain
swiftvm exec muxy-macos -- xcrun metal --version
rm target/swiftvm/Xcode.tar
```

Then the tools, no sleep, and the desktop:

```sh
swiftvm exec muxy-macos --root -- bash -c 'mkdir -p /opt/homebrew && chown muxy:admin /opt/homebrew'
swiftvm exec muxy-macos --timeout 3600 -- bash -s < "$S/setup/setup-toolchains.sh"
swiftvm exec muxy-macos --root -- pmset -a sleep 0 displaysleep 0 disksleep 0
swiftvm exec muxy-macos -- bash -s < "$S/setup/prepare-desktop.sh"
swiftvm exec muxy-macos -- sysadminctl -screenLock status   # must be off
```

Zig on macOS is Homebrew's `zig` formula. The official Zig archive fails to
link against recent Apple SDKs. Then grant the folder access described at the
top of this file.

### Windows

```sh
python3 - <<'PY'
import json, subprocess
from pathlib import Path
c = json.loads(Path('target/swiftvm/credentials.json').read_text())
subprocess.run(['swiftvm', 'create', 'windows', '--name', 'muxy-windows', '--cpus', '8',
    '--memory', '16', '--disk', '160', '--compressed', '--user', c['user'],
    '--password', c['password']], check=True)
PY
swiftvm wait muxy-windows --timeout 7200
swiftvm cp "$S/setup/setup-windows.ps1" 'muxy-windows:C:\Users\Public\setup-windows.ps1'
swiftvm exec muxy-windows --root --timeout 5400 -- powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'C:\Users\Public\setup-windows.ps1' -Part machine
swiftvm exec muxy-windows --timeout 3600 -- powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'C:\Users\Public\setup-windows.ps1' -Part user
```

SwiftVM downloads the ISO, installs unattended and signs `muxy` in
automatically. `exec` runs as whoever is signed in at the screen, and
`--root` as SYSTEM. The machine part may run again; installers that are
already there are skipped. The display driver is display-only, so GPUI draws
through WARP.

### Source and build

For each baseline:

```sh
swiftvm exec "$VM" -- mkdir -p .local/bin
for h in muxy-vm-build muxy-vm-run; do swiftvm cp "$S/guest/$h" "$VM:.local/bin/$h"; done
swiftvm exec "$VM" -- chmod +x .local/bin/muxy-vm-build .local/bin/muxy-vm-run
python3 "$S/export-source.py" "$PWD" target/swiftvm/source.tar
swiftvm cp target/swiftvm/source.tar "$VM:/tmp/muxy-source.tar"
swiftvm exec "$VM" -- bash -s /tmp/muxy-source.tar < "$S/guest/import-source.sh"
swiftvm exec "$VM" --timeout 7200 -- bash .local/bin/muxy-vm-build
```

On Windows, copy the four `.ps1` helpers from `$S/guest` to
`C:\Users\muxy\.local\bin`, the archive to `C:\Users\muxy\muxy-source.tar`,
and run `import-source.ps1 -Archive ...` and `muxy-vm-build.ps1` with
`powershell.exe -NoProfile -ExecutionPolicy Bypass -File`.

All three `--build-info` lines must match. Launch the app with
`nohup ~/.local/bin/muxy-vm-run &` and check a terminal works in an Agent
Desktop screenshot.

### Clean and save

Copy out any evidence, quit the app, then:

```sh
swiftvm exec "$VM" -- bash -c 'env MUXY_DIR="$HOME/.local/state/muxy-vm" "$HOME/.cache/muxy-vm/target/debug/muxy" server stop --force'
swiftvm exec "$VM" -- bash -ec 'test -f "$HOME/.local/share/muxy-vm/managed"; if pgrep -x muxy-app; then exit 1; fi; rm -rf "$HOME/.local/state/muxy-vm" "$HOME/e2e" "$HOME"/src/muxy-previous.*; rm -f "$HOME/.local/share/muxy-vm/app.pid" /tmp/muxy-source.tar'
swiftvm stop "$VM"
```

On Windows, quit with `muxy-e2e.ps1 stop-all`, then delete
`~\.local\state\muxy-vm`, `~\e2e`, `~\src\muxy-previous.*` and
`~\muxy-source.tar`.

Keep toolchains, caches, helpers and source. `swiftvm info` must show no
shared folders and the clipboard not shared. Each Linux machine's
`config.json` (at `Location` in `swiftvm info`) must have
`disks[].format: compressed`. Save with
`swiftvm save-template muxy-linux-wayland muxy-ubuntu-wayland` (and
`muxy-linux-x11 muxy-ubuntu-x11`, `muxy-macos muxy-macos`). If the name is
taken, use `<name>-new` and swap as in "How to refresh". Accept each new
template with step 7 of "How to refresh", then leave the baselines stopped.
