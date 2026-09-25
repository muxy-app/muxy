# Constraints

Platform requirements and implementation constraints. Performance observations
come from the historical [benchmarks](./benchmarks.md).

## Platform

- The desktop supports macOS 14+. Standalone CLI/server targets are macOS 14+
  and Linux with glibc 2.35+, each on x86_64 and ARM64. Linux support requires
  native build and runtime verification on both architectures.
- Server paths are Unix pathname bytes. Local clients use Unix sockets and
  paired phones use TLS over TCP on IPv4. Desktop and TUI remote transport,
  native Windows, and musl/Alpine support are deferred.
- The mobile SDK builds for iOS devices and simulators, and for Android, with
  `scripts/build-mobile-sdk.sh`. During the beta a phone and its server must
  share the compatibility identifier, so phone releases follow its bumps.
- Turning on mobile access may show the macOS firewall prompt for
  `muxy-server`, and the iOS app needs local-network permission to reach LAN
  addresses. `MUXY_REMOTE_BIND` limits the listener to one IPv4 address; tests
  use loopback.

## Ghostty terminal core

- Build with `LIBGHOSTTY_VT_SYS_OPTIMIZE=ReleaseFast`, as configured in the
  repository. Debug integrity checks made the measured native build too slow.
- The terminal is not sendable and the C API is not thread-safe. One thread
  owns each terminal; everything else talks to that thread.
- `max_scrollback` is a byte budget, matching the product's retention setting.
- Compression is caller-driven. The server decides when a session is idle
  and calls one full pass; decompression on access is transparent.
- Physical footprint, not RSS, is the metric that reflects compression,
  because released pages are `madvise`d rather than freed.
- The crate API is marked unstable. Pin the version and wrap it behind one
  module.

## PTY

- The kernel charges per producer write on both sides of a pty. A
  line-at-a-time producer caps near 12 MB/s on macOS and forces one read
  syscall per line; a block writer moves 280 MB/s through the same pty.
- Reads therefore arrive small and frequent for chatty programs. The reader
  must be a dedicated blocking thread that only reads and forwards; sleeping
  to batch reads stalls the producer because the kernel pty buffer is only
  a few kilobytes.
- In the measured workloads, producer write patterns dominated the CPU budget;
  engine parse speed was not the bottleneck.

## Sockets and processes

- On macOS an accepted Unix socket inherits the listener's non-blocking
  flag. Set blocking explicitly on every accepted stream.
- A queue that is not bounded by merging grows by the full output rate
  whenever a client stalls. Merging per channel bounds pending screen state.

## Rendering

- Redraw on demand avoids idle work.
- Per-cell colour churn is the pathological case. Merge quads by colour and
  skip shaping for blank runs before adding caches.
- In the spike, shaped-line caching did not help; paint submission dominated.

## Measurement

- The workloads, metrics, and hard-fail rules in
  [benchmarks.md](./benchmarks.md) are the regression baseline. Re-measure
  after any change to the engine, the frame shape, or the flow control.

## Shell startup

The server installs private hooks beside its socket. zsh and fish load them
without editing user startup files. Disabling shell integration in Settings applies
to new sessions immediately; manual changes to `server.toml` require a restart.
Shell-native integration, such as fish 4's prompt marks, is left alone.

Bash keeps its normal login startup. To opt in, source the hook from the
interactive startup file your Bash profile loads:

```bash
if [[ ${MUXY_SHELL_INTEGRATION:-0} == 1 ]]; then
    source "$MUXY_SHELL_INTEGRATION_DIR/muxy.bash"
fi
```

An existing Bash DEBUG trap is preserved; command-start and exit-status marks
are omitted in that case, but prompt navigation still works.
