# Session protocol

Muxy's optional persistent terminal runtime uses the `muxy-session` helper and a private Unix domain socket. The protocol is independent from the app's P2 command socket and does not accept retained Swift session data.

## Transport and authentication

Every connection uses a fixed 24-byte big-endian header with magic `MXS8`, protocol major and minor versions, frame kind, flags, request ID, and payload length. The receiver validates the complete header and the frame-kind-specific payload limit before allocating payload storage.

The daemon authenticates the operating-system peer before reading the first frame. macOS uses `LOCAL_PEERCRED` plus `LOCAL_PEERPID`. Linux uses `SO_PEERCRED`. The `Hello` PID must equal the authenticated peer PID.

Every client sends `Hello` with its protocol version, client kind, PID, and a random nonce. The daemon returns `HelloAccepted` with the negotiated version, daemon PID and start identity, daemon nonce, and build mode. A major-version mismatch returns `VersionMismatch` and closes the connection. A build-mode mismatch fails explicitly and never starts a replacement daemon. Incomplete handshakes and missing first operations time out and release their connection permits.

## Limits

- Structured payload: 1 MiB
- Input or output chunk: 32 KiB
- Pending input per session: 1 MiB
- Pending output for the active renderer: 4 MiB
- Simultaneous control connections: 64
- Replay per session: 256 KiB

Creation requests also bound argument counts, environment counts, individual and aggregate values, paths, terminal dimensions, and structured payload size. Invalid paths, NUL bytes, duplicate environment keys, invalid environment names, and unsupported sizes fail before PTY creation.

## Control operations

Control clients use request-correlated messages:

- `ListSessions`
- `GetSession`
- `CreateSession`
- `EndSession`
- `EndSessionsByOwner`
- `EndAllSessions`
- `SetWorkspacePlacement`
- `Ping`

Creation is idempotent by immutable project, worktree, and original-tab ownership plus the launch contract. An owner with a different launch contract returns a duplicate-owner conflict.

## Renderer lifecycle

A renderer sends `Attach` after `Hello`. The daemon assigns a unique attachment generation, applies the initial size before returning `Attached`, sends bounded replay, then sends ordered live output. Input frames are bounded. Resize frames carry the daemon-assigned attachment generation and a renderer resize generation, so an old renderer cannot resize or detach its replacement.

Each session supports one active renderer. A new attachment deterministically replaces and closes the previous attachment. Renderer disconnect is detach, not shell exit. PTY draining continues without a renderer. If a renderer exceeds the pending-output limit, that attachment is closed while bounded replay and the daemon-owned shell continue.

Replay omits bytes from an active alternate screen. Entering or leaving alternate screen starts a new replay generation. Retained bytes are bounded and begin and end at safe terminal-stream boundaries.

## Runtime paths

The app and helper use an explicit profile-specific `control.sock` path selected by `RuntimePathPolicy`. The runtime leaf is effective-UID-owned mode `0700`. The socket, singleton lock, and daemon log are mode `0600`. The trusted parent is canonicalized once, the owned leaf and private files are opened descriptor-relatively with no-follow behavior, and the held directory identity is revalidated around socket recovery and binding. A daemon holds a nonblocking exclusive lock for its lifetime. It removes only a private stale socket while holding that lock and never unlinks a live socket.

Development and production use separate path names, locks, logs, sockets, and build-mode handshakes. Tests use only unique `/tmp/p8-isolated-test-*` or verifier-owned roots. They never discover or clean processes by executable name.

## PTY and process ownership

The daemon owns the real shell PTY. The PTY root must stabilize as its own process group, process session, foreground PTY group, and nonzero TTY identity before supervision starts. Process supervision records PID plus start identity and continuously discovers descendants through session, process-group, TTY, and parent relationships.

Ending a session sends signals only after revalidating each recorded PID/start identity. It rescans during a bounded TERM grace period, escalates surviving recorded identities to KILL, reaps the direct child, and acknowledges only after the tracked tree is gone. A reused PID is never adopted or signaled.

Natural shell exit drains remaining PTY output, performs the same exact descendant cleanup, records the final status, and then marks the descriptor ended. Renderer loss, control connection loss, daemon unavailability, and shell exit are separate states. A client cannot infer shell exit from an attach transport failure.

The daemon exits after ten seconds with no running sessions or connections. It does not use a launch agent, login item, or installed background service. The helper exposes only:

```text
muxy-session daemon --socket PATH
muxy-session attach --socket PATH --session-id UUID
```
