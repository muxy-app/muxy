# Constraints

Supported platforms, and lessons that shape the code.

## Platforms

| Part | Supported |
| --- | --- |
| Desktop app | macOS 14 and later |
| `muxy` and `muxy-server` | macOS 14 and later, and Linux with glibc 2.35 or later, on x86_64 and ARM64 |
| Mobile SDK | iOS and Android |

Not yet supported: Windows, musl or Alpine Linux, and desktop app connections
to other machines. Linux builds must be built and run natively on both
architectures to count as supported.

On Linux hosts that end a user's processes at logout, run
`loginctl enable-linger` so the server keeps running after an SSH session ends.

Turning on mobile access may show a macOS firewall prompt for `muxy-server`, and
the iOS app needs local network permission.

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
  A program that writes line by line tops out near 12 MB/s on macOS. One that
  writes large blocks reaches 280 MB/s through the same PTY.
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
