# GhosttyKit

Muxy depends on libghostty compiled as a static library inside `GhosttyKit.xcframework/`. The xcframework is built and released via GitHub Actions on the [muxy-app/ghostty](https://github.com/muxy-app/ghostty) fork.

## Local Setup

```bash
scripts/setup.sh
```

This downloads the latest pre-built `GhosttyKit.xcframework` from the fork's releases and syncs the header into `GhosttyKit/ghostty.h`.

## Rebuilding GhosttyKit

To build a new version of the xcframework (e.g. after ghostty updates):

1. Go to [muxy-app/ghostty Actions](https://github.com/muxy-app/ghostty/actions)
2. Run the "Build GhosttyKit" workflow
3. Once complete, re-run `scripts/setup.sh` locally (delete the old xcframework first)

```bash
rm -rf GhosttyKit.xcframework
scripts/setup.sh
```

## How it works

1. The fork's "Build GhosttyKit" workflow builds libghostty with Zig on a macOS runner
2. It produces a universal xcframework (arm64 + x86_64) and publishes it as a GitHub release
3. `scripts/setup.sh` downloads the latest release and extracts it
4. `Package.swift` links against `GhosttyKit.xcframework/macos-arm64_x86_64/libghostty.a`

## Syncing the fork

The fork auto-syncs from upstream ghostty daily via the "Sync Upstream" workflow.

## Muxy-specific patches

The fork carries two additive exports used by the remote-server integration. Both live near `ghostty_surface_text` in the embedded apprt so
they're easy to keep on top of upstream:

- `ghostty_surface_set_data_callback(surface, cb, userdata)` — registers a
  per-surface callback that fires on the termio thread with raw PTY bytes
  before Ghostty's emulator parses them. Used by `RemoteTerminalStreamer` to
  tee output to remote clients.
- `ghostty_surface_send_input_raw(surface, ptr, len)` — writes bytes straight
  to the PTY, bypassing the paste pipeline (no bracketed-paste wrapping, no
  newline filtering). Used by `GhosttyTerminalNSView.sendRemoteText` so
  remote-client keystrokes, escape sequences, and mouse reports reach the
  child process verbatim.

The patch touches three Zig files (`src/Surface.zig`, `src/termio/Termio.zig`,
`src/apprt/embedded.zig`) plus `include/ghostty.h`. Everything is strictly
additive except a three-line tee at the top of `Termio.processOutput`.
