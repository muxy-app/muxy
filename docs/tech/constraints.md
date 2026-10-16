# Constraints

Supported platforms, and lessons that shape the code.

## Platforms

| Part | Supported |
| --- | --- |
| Desktop app | macOS 14 and later |
| `muxy` and `muxy-server` | macOS 14 and later, and Linux with glibc 2.35 or later, on x86_64 and ARM64 |
| Mobile SDK | iOS and Android |

Not yet supported: Windows, and musl or Alpine Linux. Linux builds must be
built and run natively on both architectures to count as supported.

### Experimental Linux desktop

The Linux desktop port is under development. It is tried on Ubuntu 24.04 ARM64
in SwiftVM on Apple Silicon, with GNOME and KDE Plasma, each on Wayland and X11.
This does not establish support for other distributions, x86_64 desktop builds,
or hardware GPU drivers. SwiftVM uses Mesa's software Vulkan renderer, so its
performance is not representative of a native GPU.

The system draws the window: frame, buttons, shadow, resizing and tiling. GNOME
on Wayland draws no frame, so there the app shows its own buttons and the
window cannot be resized from its edges.

The port covers the terminal desktop, in-app dialogs and menus, desktop-portal
file selection, desktop notifications, Linux shortcuts, and `xdg-open` for
external resources. Commands that macOS offers in the menu bar are in the
command palette. Copy and paste use Ctrl+Shift+C and Ctrl+Shift+V in terminals;
text fields use Ctrl+C and Ctrl+V. Links in a terminal open with Ctrl+click.
Existing configured shortcuts remain literal. New configurations use the system
monospace font and inline paths for composer images.

One app runs per profile: a second launch, and `muxy <folder>`, hand their
folders to the app that is running. The app, the `muxy` tool and the server are
installed in one directory and find each other there.

Extensions run on the system's WebKitGTK 4.1, version 2.40 or later, which the
app loads once an extension is enabled. Without it the app still runs, and
Settings says why extensions are off. Scripts use its JavaScriptCore, the engine
macOS uses. A GPUI window cannot hold a WebKitGTK view, so pages render off
screen and the app draws them and passes them input. Browser panes use the same
renderer and need WebKitGTK 2.42 or later for cookie automation. On Linux a page therefore
has:

- no context menu, tooltips, or drag and drop;
- text only on the clipboard, and no clipboard inside a frame;
- no text from an input method that composes it, as for Chinese or Japanese;
- an arrow, a hand, or a text cursor, and no other pointer shape;
- content processes outside WebKit's sandbox where the system forbids the
  sandbox, as Ubuntu 24.04 does for an app without an AppArmor profile.

On Ubuntu, download `muxy-desktop.apparmor` from the
[latest stable release](https://github.com/muxy-app/muxy/releases/latest) to allow
WebKit's sandbox at the default installation paths for all three Linux formats.
An administrator can install it:

```sh
sudo install -m 644 muxy-desktop.apparmor /etc/apparmor.d/muxy-desktop
sudo apparmor_parser -r /etc/apparmor.d/muxy-desktop
```

Adjust its attachment paths for a custom `XDG_DATA_HOME`, then fully quit and
reopen Muxy. The profile permits user-namespace creation; it does not confine
the whole terminal app. Installers do not change system security settings.
Remove the optional profile only when no installed channel or format needs it.

Rendering needs a working Vulkan 1.1 driver and the extensions required by
GPUI's Blade renderer. Use your GPU vendor's driver and your desktop's portal
backend: `xdg-desktop-portal-gnome`, `xdg-desktop-portal-kde` or
`xdg-desktop-portal-gtk`.

SF Symbols exist only on macOS. Linux draws a symbol from a bundled icon when
there is one, and an extension's symbol without one as the extension icon.

Quick Terminal has an experimental X11 backend. Its global shortcut is released
while recording a replacement, and hiding the window keeps its terminal session.
Wayland and XWayland report it unavailable: the pinned window backend cannot
place and hide a dropdown or use a portal activation token, and XWayland grabs
do not cover native Wayland apps. The macOS panel is unchanged.

Voice input uses local `parec` capture and `whisper-cli` transcription. Install
PulseAudio's utilities (`pulseaudio-utils` on Ubuntu) and
[whisper.cpp](https://github.com/ggml-org/whisper.cpp), v1.9.5 or later. The
audio service must expose its local PulseAudio socket, as PipeWire's PulseAudio
compatibility service does. Muxy records the default input device; choose it
and control access in the desktop's sound settings. There is no separate
microphone permission dialog in this backend.

Download a multilingual GGML model, such as `ggml-base.bin` (about 142 MiB),
from [whisper.cpp's model collection](https://huggingface.co/ggerganov/whisper.cpp).
Select its absolute path in Settings → Composer → Offline voice model, or put
it at `$XDG_DATA_HOME/muxy/voice/ggml-base.bin` (by default
`~/.local/share/muxy/voice/ggml-base.bin`). English-only models are rejected so
they cannot silently override the selected language.

Also install the [Silero](https://github.com/snakers4/silero-vad) speech
detection model
[`ggml-silero-v6.2.0.bin`](https://huggingface.co/ggml-org/whisper-vad/resolve/main/ggml-silero-v6.2.0.bin)
(885,098 bytes) at `$XDG_DATA_HOME/muxy/voice/ggml-silero-v6.2.0.bin`, or
`~/.local/share/muxy/voice/ggml-silero-v6.2.0.bin` when `XDG_DATA_HOME` is unset.
Its SHA256 is
`2aa269b785eeb53a82983a20501ddf7c1d9c48e33ab63a41391ac6c9f7fb6987`.
This model is required even when the voice model is stored elsewhere. Muxy
checks for it before opening the microphone and uses it to discard audio
without detected speech before transcription. This reduces invented text from
silence or keyboard noise; speech detection and recognition can still make
mistakes. Model downloads are manual; Muxy does not download anything for
dictation.

Audio stays in memory and is sent through a pipe to the local model when the
user finishes. There is no live transcript or network transcription. Pause
releases the microphone, and Cancel stops capture or transcription. A recording
pauses at ten minutes, keeping its audio available for Finish. Transcription
uses at most four CPU threads and times out after ten minutes. One dictation
session can run in the app at a time. macOS keeps its native speech service and
resolves language selections imported from Linux to installed speech locales.

Linux releases installed with the user-local installer use a versioned installation
with an atomic `current` link, verified downloads and the shared session-compatibility
rules. Debian packages are managed by APT; AppImages are replaced manually. Neither
uses the in-app updater. AppImages retain a private, unpacked copy so background
servers and CLI commands do not depend on a temporary FUSE mount.
See [Linux desktop installation and updates](../user-guide/linux.md). The
package baseline is Ubuntu 24.04 and glibc 2.39; this does not change the
standalone CLI/server floor of 2.35.
Clipboard image submission is unavailable because the current
clipboard API cannot safely preserve all MIME types and detect ownership
changes. Use inline image paths instead.

To build on Ubuntu, install Rust from `rust-toolchain.toml`, Zig 0.15.2, and the
development packages `build-essential pkg-config clang cmake libssl-dev
libfontconfig1-dev libfreetype-dev libxkbcommon-dev libxkbcommon-x11-dev
libwayland-dev libxcb1-dev libvulkan-dev`. Runtime packages include
`mesa-vulkan-drivers xdg-desktop-portal xdg-desktop-portal-gnome xdg-utils`, and
`libwebkit2gtk-4.1-0` for extensions.
The desktop checks also need `xvfb` and `xauth` for isolated X11 shortcut and
window-lifecycle tests.
Then run `bash scripts/build-desktop-linux.sh`. `MUXY_ZIG` can point at Zig, and
`CARGO_TARGET_DIR` must point to a guest-local writable directory when the source
checkout is shared read-only. Keep `muxy-app`, `muxy`, and `muxy-server` together.

Visual checks are performed by a person: start the app from a desktop terminal,
open a project, create and split terminals, exercise shell input and Ctrl+C,
copy/paste Unicode text, scroll history, open Settings and a folder picker, then
cancel a close confirmation before confirming another. Check both Wayland and
X11 sessions. Next milestones are native x86_64 and hardware GPU validation,
package installation and upgrade acceptance, and the deferred platform backends.

On Linux hosts that end a user's processes at logout, run
`loginctl enable-linger` so the server keeps running after an SSH session ends.

Turning on mobile access may show a macOS firewall prompt for `muxy-server`, and
the iOS app needs local network permission.

macOS charges a process's privacy permissions, such as Local Network, to the app
process that started it. The desktop app starts the server through a small copy
of itself that lives as long as the server, so terminals keep Muxy's permissions
after the app quits.

## Terminal engine

- Ghostty's terminal isn't thread-safe, so one thread owns each terminal.
- The history limit is in bytes.
- The server decides when to compress history, once a session goes quiet.
- Measure memory as physical footprint, not RSS. Compressed pages are released
  to the system but not freed, so RSS doesn't show the savings.
- Ghostty's Rust API is unstable. Pin its version and keep it inside
  `muxy-terminal`.

## PTYs

- How fast a terminal fills up depends on how the program writes, not on parsing.
  In our benchmarks, a program that writes line by line reached about 12 MB/s
  on macOS. One that writes large blocks reached 280 MB/s through the same PTY.
- Reads therefore arrive small and often. The reader must be its own blocking
  thread. Sleeping to batch reads would stall the program, because the kernel's
  PTY buffer holds only a few kilobytes.

## Memory and drawing

- A queue that grows with output explodes when an app stalls. Merge instead of
  queueing ([D8](./decisions.md#d8-one-merged-frame-in-flight)).
- Redraw only when something changes. Heavy per-cell color changes are the worst
  case: merge rectangles by color and skip blank text before reaching for caches.
- Measure again after changing the terminal engine, the screen format, or flow
  control.

## Shell integration

The server loads its shell hooks for zsh and fish automatically, without
editing your startup files. Bash is opt-in. Add this to the file your Bash
profile loads for interactive shells:

```bash
if [[ ${MUXY_SHELL_INTEGRATION:-0} == 1 ]]; then
    source "$MUXY_SHELL_INTEGRATION_DIR/muxy.bash"
fi
```

If you already have a Bash `DEBUG` trap, it is kept. Jumping between prompts
still works, but command start and exit status aren't marked. Turning shell
integration off in Settings applies to new terminals right away.
