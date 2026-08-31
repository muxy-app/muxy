# P8 Terminal Memory and Background Sessions Implementation Plan

> **Self-contained execution contract:** This plan contains the approved decisions, evidence, file destinations, phase order, gates, and manual checks required to implement P8 without relying on the planning conversation. Run phases in order, one at a time. Do not begin a later phase until the current phase is green and the staged app has launched and closed normally. Cited line numbers were valid at planning time; if code has drifted, re-locate the cited symbol before editing. Update **Progress** after every phase with exact commands, observed results, failures, and unverified behavior.

## Context

`PLAN.md` requires a detailed plan before each implementation phase. P8 is the next Stage B phase after P7 and covers terminal surface memory, process-tree resource reporting, and persistent sessions (`PLAN.md:171-187`). P6 and P7 manual acceptance still tracked in their plans does not block P8 planning.

P8 has three related but independent concerns:

1. Idle sleeping frees hidden Ghostty renderer surfaces when the separate idle setting permits it.
2. Resource monitoring measures the Muxy app tree, the session daemon, daemon-owned shells, and their descendants.
3. Optional background persistence moves local workspace PTY and shell ownership into a Rust daemon so tabs continue running after Muxy quits. Ghostty becomes an attachable renderer and detached sessions have no Ghostty surface.

The retained Swift implementation is evidence for user-visible behavior, test cases, shell transformation, and process rules. It is not lifecycle or protocol authority because its background-session implementation is incomplete. P8 uses a clean Rust protocol and does not preserve Swift wire compatibility.

The current Rust seams establish the starting point:

- The workspace has eight crates and no session daemon crate (`Cargo.toml:1-12`).
- `RuntimePathPolicy` already owns mode-specific `sessions-dev` and `sessions` directories, `control.sock`, and UID-specific fallback paths (`crates/muxy-core/src/environment.rs:60-135`).
- Typed terminal tabs have no Rust session ID (`crates/muxy-core/src/workspace/tab.rs:16-47`). Raw unknown workspace fields are preserved by the workspace store, so retained `paneSessionID` can remain untouched without being interpreted (`crates/muxy-core/src/workspace_store.rs:413-456`, `crates/muxy-core/src/workspace_store.rs:739-789`).
- Hidden terminal handles are retained and only occluded today (`crates/muxy/src/terminal/surfaces.rs:517-599`).
- A surface exit currently closes its workspace tab, which does not distinguish renderer or attach-proxy failure from daemon-owned shell exit (`crates/muxy/src/views/window/terminal.rs:63-98`).
- The terminal backend already exposes neutral input, PID, cells, focus, and occlusion seams (`crates/muxy-terminal/src/backend.rs:111-140`).
- Rust wrappers already cover Ghostty occlusion, foreground PID, raw input, and cell reads (`crates/ghostty-host/src/surface.rs:375-409`, `crates/ghostty-host/src/surface.rs:450-453`, `crates/ghostty-host/src/surface.rs:569-587`).
- `ghostty_surface_set_data_callback` exists in the vendored header, but retained Swift uses it for raw-output/mobile streaming rather than persistent attachment (`Muxy/Views/Terminal/GhosttyTerminalNSView.swift:1945-1962`).
- Background persistence and idle sleeping are mirrored as default-off settings, the idle timeout defaults to 300 seconds, and the resource status setting is default on (`crates/muxy-core/src/prefs/settings.rs:71-95`, `crates/muxy-core/src/prefs/settings.rs:139-141`).
- `list-sessions` and `kill-session` are already accepted P2 catalog heads reserved for P8 (`crates/muxy/src/socket/catalog.rs:86-90`).
- The status bar has one trailing element, and P7 Composer already uses it, so P8 must compose trailing items instead of replacing Composer (`crates/muxy/src/views/status_bar.rs:18-29`, `crates/muxy/src/views/status_bar.rs:115-128`, `crates/muxy/src/views/app.rs:245-295`).
- The bundle builder stages only `muxy`, while the bundle verifier expects only that executable (`scripts/build-app.sh:77-117`, `scripts/verify-bundle.sh:27-55`).
- The full `resources/ghostty` tree is already copied into the bundle, including zsh, bash, fish, nushell, and elvish integrations. The verifier currently checks only bash, fish, and zsh (`scripts/build-app.sh:115-133`, `scripts/verify-bundle.sh:85-116`).
- `scripts/check.sh` enforces crate boundaries, formatting, Clippy, workspace build/tests, prior phase verifiers, and a debug bundle, but has no P8 verifier (`scripts/check.sh:183-200`, `scripts/check.sh:260-296`).

### Normative source order

When sources conflict, use this order:

1. The numbered locked decisions below.
2. This resolved design and phase contract.
3. P8 scope and placement rules in `PLAN.md`, except where a locked decision explicitly supersedes older Swift-protocol language.
4. Retained Swift behavior and tests for details not changed above.
5. Current Rust architecture and reusable patterns.

Planning was read-only except for this plan file. No build, test, bundle, app launch, daemon survival, process cleanup, or Linux cross-target result is claimed here.

## Locked decisions

1. **P8 is next.** P6 and P7 remaining manual acceptance does not block P8 planning.
2. **Swift background sessions are not lifecycle authority.** The retained implementation is incomplete.
3. **Use a tmux-like ownership model.** The daemon owns each persistent PTY and shell independently of the GUI, sessions continue while Muxy is entirely closed, Ghostty is an attachable renderer/client, and detached sessions have no Ghostty surface.
4. **Daemon backing covers all local workspace terminal tabs when enabled.**
5. **Quick Terminal is excluded from background persistence.**
6. **Remote terminals are excluded from background persistence.**
7. **Background persistence is user-controlled and default off.**
8. **Changing persistence mode requires Quit and reopen.** Do not live-adopt or live-convert PTYs.
9. **Enabling does not adopt current in-app terminal processes.** Disclose that ordinary terminals will be replaced by new daemon-backed sessions after Quit and reopen.
10. **Confirmed disabling ends all daemon-owned sessions and child processes on the next launch before ordinary workspace terminals reopen.**
11. **Relaunch restores Ghostty surfaces only for currently visible split panes.**
12. **Hidden workspace tabs stay daemon-backed and surface-free until selected.**
13. **Persistence and idle sleeping are separate controls.** Persistence owns PTY survival. Idle sleeping owns hidden renderer removal.
14. **Idle sleeping remains default off and retains the existing timeout choices.**
15. **Close Tab ends the backing session after the existing running-process confirmation.**
16. **Send to Background removes the tab from the workspace while keeping the daemon session detached.**
17. **The session manager is a status-bar popover.**
18. **The manager lists workspace-attached and detached sessions and supports Focus, Reattach, End, End All, and Settings access.**
19. **Reattach returns a detached session to its original project and worktree.**
20. **App-driven project or worktree deletion discloses and terminates corresponding background sessions before deletion continues.**
21. **External project or worktree disappearance makes corresponding sessions orphans, which reconciliation terminates.**
22. **Ordinary Quit detaches renderers and leaves daemon sessions running.**
23. **End All Sessions is a separate confirmed action.**
24. **Use a clean, versioned Rust protocol.**
25. **Do not preserve Swift wire compatibility, dual protocols, old-session import, old-daemon adoption, or `paneSessionID` adoption.**
26. **Protocol mismatch fails explicitly and safely.**
27. **P8 scope is terminal and session work only:** idle sleeping, process-tree monitoring and status UI, tmux-like runtime and UX, CLI activation, security, profile isolation, packaging, tests, guardrails, and documentation.
28. **Exclude unrelated Stage B extras:** general quit confirmation, generic relaunch-in-place, deferred startup ordering, and `ApplePressAndHoldEnabled` or tooltip defaults.
29. **Execute incrementally.** Every phase ends with focused tests, full gates, a verified bundle, staged app launch and normal close, and process cleanup proof.
30. **Longest-lived constraints are portable protocol, policy, and model boundaries, stable profile/build-mode names, and target-gated PTY, peer-credential, proc/sysctl, and Ghostty code.**
31. **Linux cross-target compilation is required.** Linux-host launch remains P15.
32. **P2 remains stable.** Socket framing, accepted-head catalog, retained CLI source bytes, and the legacy CLI bundle path do not change.
33. **Resource monitoring covers the Muxy process tree, daemon/session processes, and child process trees.**
34. **The resource widget retains its existing default-on behavior.**
35. **No global mutable singleton state.**
36. **Repository rules remain binding.** Add no code comments, Python scripts, lint-suppressing `allow` or `expect`, caller-specific policy in shared code, or unrequested git history, worktree, commit, or publish activity.

## Resolved design details

### 1. Crate ownership and dependency direction

Add a ninth crate, `crates/muxy-session`, with a library for integration tests and one `muxy-session` binary. Keep this acyclic graph:

```text
muxy-proto

muxy-core -> muxy-proto
muxy-terminal -> muxy-core
muxy-session -> muxy-proto + muxy-core
muxy -> muxy-session + muxy-api + muxy-proto + muxy-core + muxy-terminal
muxy-terminal -> ghostty-host -> ghostty-sys on macOS
```

Ownership is strict:

- `muxy-proto` owns `SessionId`, owner IDs, descriptors, protocol versions, bounded frame encoding/decoding, control and attach messages, replay buffering, terminal-stream parsing, and window-size validation. It remains independent of core, app, GPUI, Ghostty, and platform PTY code. Its current independent boundary is visible in `crates/muxy-proto/Cargo.toml:1-22` and enforced by `scripts/check.sh:191-195`.
- `muxy-core` owns typed tab/session linkage, portable transition and owner-reconciliation plans, and resource identities/aggregation math. Adding `muxy-core -> muxy-proto` is safe because `muxy-proto` has no reverse dependency.
- `muxy-terminal` owns the offline timeout/policy/state machine, neutral renderer lifecycle, terminal activity facts, and target-gated foreground/process safety facts. This follows the explicit P8 placement rule (`PLAN.md:242-250`) rather than placing idle policy in core.
- `muxy-session` owns daemon lifecycle, attach proxy, same-user Unix transport, secure runtime paths, PTY/shell ownership, replay, the single renderer attachment, full process-tree termination, macOS/Linux adapters, and one narrow public client facade used by the app and attach proxy. Daemon internals remain private.
- `muxy` depends on that single `muxy-session` client facade and owns one injected `SessionCoordinator`, startup reconciliation, app settings effects, Ghostty launch selection, workspace actions, deletion integration, session manager, one resource monitor, and P2 command dispatch. Do not add a second app-owned transport client.
- `ghostty-host` remains an FFI-only wrapper. It may expose neutral cell and activity facts, not app session policy.

Do not add a reusable abstraction unless P8 code has at least one current caller. In particular, do not introduce P10 extension-host resource policy or a generic daemon framework.

### 2. Versioned daemon protocol

Use a separate binary protocol over a mode-specific Unix domain socket. It does not reuse P2 line/NUL framing.

Use a fixed 24-byte big-endian header:

```text
magic       [u8; 4] = "MXS8"
major       u16
minor       u16
frame_kind  u16
flags       u16
request_id  u64
payload_len u32
```

Structured control payloads use typed UTF-8 JSON. Terminal `Input` and `Output` frames use bounded binary bytes, with output carrying a monotonic sequence number. Validate the header and payload length before allocation.

Initial limits:

- Structured frame: 1 MiB.
- Input or output chunk: 32 KiB.
- Pending input per session: 1 MiB.
- Pending output for the active renderer: 4 MiB.
- Simultaneous control connections: 64.
- Replay per session: 256 KiB.

Every connection begins with `Hello`. The client supplies protocol version, client kind, PID, and a nonce. The daemon returns either `HelloAccepted` with negotiated version, daemon PID/start identity, daemon nonce, and build mode, or an explicit `VersionMismatch`. Major mismatch is fatal. Minor negotiation is allowed only for messages available in the negotiated minor. Same-user authentication occurs before frame decoding.

Control operations are:

- `ListSessions`
- `GetSession`
- `CreateSession`
- `EndSession`
- `EndSessionsByOwner`
- `EndAllSessions`
- `SetWorkspacePlacement`
- `Ping`

`CreateSessionRequest` is a complete bounded launch contract. It carries the proposed Rust session ID; immutable `(project_id, worktree_id, original_tab_id)` idempotency key; current tab placement; cwd; initial rows/columns; shell executable and argv; optional one-shot startup command and keep-open behavior; sanitized environment entries; Ghostty resource and terminfo paths; terminal/color values; and title. Limit it to 256 argv entries, 512 environment entries, 256-byte environment keys, 64 KiB individual argument/environment values, 256 KiB aggregate argv, and the existing 1 MiB structured-frame limit. Reject NULs, duplicate environment keys, invalid names, non-absolute executable/resource paths, invalid cwd, and out-of-range sizes. The app obtains inherited environment only through `muxy_api::execution_environment`, scrubs every old/new Muxy-reserved key, and sends this request. The daemon takes no unrestricted environment snapshot and independently validates every field before PTY creation.

The immutable owner tuple is unique within one mode-specific current Rust daemon. `CreateSession` returns the existing matching current-protocol descriptor when both owner tuple and launch fingerprint match, returns a typed duplicate-owner conflict when they differ, and never matches retained Swift data or another build mode.

`SessionDescriptor` carries Rust session ID, immutable project/worktree/original-tab owner IDs, current workspace tab placement or none, title, cwd, shell PID/start identity, process-session/group and TTY identities, creation time, renderer-attached state, and running/exited status.

Attach flow is `Hello`, `Attach`, `Attached`, bounded replay, then ordered live output. Client input and resize flow back to the daemon. Replay and live output are serialized by the session worker so there is no gap between them. Replay respects UTF-8 and terminal escape boundaries. Alternate-screen bytes remain live but are not retained as normal replay; entering or leaving alternate screen starts a clean replay generation. The retained replay and terminal-stream implementations provide test-vector evidence (`MuxySessionProtocol/SessionReplayBuffer.swift:3-154`, `MuxyShared/TerminalStreamSequence.swift:3-219`).

`muxy-session attach` reads the initial renderer PTY size with `TIOCGWINSZ`, handles and coalesces `SIGWINCH` through a signal-safe wakeup mechanism, and sends bounded generation-tagged `Resize` messages. The daemon applies `TIOCSWINSZ` to the real PTY and signals its foreground process group. Initial size is applied before replay, and a stale attachment generation cannot resize a replacement renderer.

Support exactly one active renderer attachment per session in P8. A new authenticated attach atomically replaces a stale prior renderer connection. Multiple simultaneous GUI renderers, fan-out, leader selection, and read-only clients are not required by any locked decision and must not be invented. Multiple control connections remain bounded and independent. PTY draining never blocks on a renderer. If its pending-output cap is reached, atomically disconnect that renderer generation, continue draining into the bounded replay ring, and keep termination/control handling on an independent high-priority path.

### 3. Secure path, daemon, PTY, and process lifecycle

Consume `RuntimePathPolicy` rather than duplicating names. Prefer the profile-specific app-support session socket and use the existing UID-specific `/tmp` fallback only when the Unix socket path is too long (`crates/muxy-core/src/environment.rs:60-135`). App and helper both receive and validate the explicit selected path.

On Unix:

- Open trusted ancestors and app-owned leaves descriptor-relatively with no-follow semantics.
- Create or validate the session leaf as effective-UID-owned mode `0700`.
- Reject symlinks, non-directories, foreign ownership, and group/world permission bits in app-owned components.
- Create lock and daemon log as `0600`; bind and chmod the socket to `0600`.
- Hold a mode-specific `flock` for daemon lifetime.
- Never unlink while another daemon may own the socket. Only while holding the singleton lock, reject symlink/non-socket entries, probe an owned socket, and unlink it if no listener exists.
- Authenticate macOS peers with `LOCAL_PEERCRED` and Linux peers with `SO_PEERCRED` before decoding.

A client that cannot connect resolves sibling `Contents/MacOS/muxy-session`, spawns `muxy-session daemon` in a new process session, and retries with a bounded deadline. Development and production use their existing distinct path names. The daemon remains alive while at least one session exists and exits after a bounded 10-second idle period when it has no sessions or connections.

The helper exposes only:

```text
muxy-session daemon --socket <validated-path>
muxy-session attach --socket <validated-path> --session-id <uuid>
```

The Unix PTY adapter validates cwd and window size, creates a controlling PTY/session, resets inherited signals, closes unrelated descriptors, installs a sanitized environment, executes the configured shell or new-terminal startup command, records PID plus start identity, performs nonblocking bounded I/O, reaps direct children, and reports exit. Rust must not call retained Swift code. Target-specific API choices stay behind adapters.

Sanitize the six Muxy environment keys already scrubbed by Ghostty (`crates/muxy/src/terminal/ghostty/mod.rs:148-159`) and every new `MUXY_SESSION_*` key before injecting current pane/project/worktree, app socket, session ID/socket, Ghostty resources, terminfo, terminal type, and color values.

Port table-driven zsh, bash, fish, elvish, and nushell environment transformations from retained test vectors (`MuxySessionProtocol/SessionShellIntegration.swift:51-135`, `Tests/MuxyTests/Services/Session/SessionShellIntegrationTests.swift:27-157`). Use the existing bundled resource inventory:

- `resources/ghostty/shell-integration/zsh/.zshenv`
- `resources/ghostty/shell-integration/zsh/ghostty-integration`
- `resources/ghostty/shell-integration/bash/ghostty.bash`
- `resources/ghostty/shell-integration/bash/bash-preexec.sh`
- `resources/ghostty/shell-integration/fish/vendor_conf.d/ghostty-shell-integration.fish`
- `resources/ghostty/shell-integration/bash/elvish/lib/ghostty-integration.elv`
- `resources/ghostty/shell-integration/nushell/vendor/autoload/ghostty.nu`

Do not rename or duplicate these resources. Resolve the actual elvish and nushell bundle paths from this existing tree.

Supervision begins at session creation, not only at termination. The daemon continuously records PID/start identities discovered through the shell session ID, process groups, controlling TTY, parent relationships, and prior known descendants. Ending a session stops input, signals PTY/session roots first, repeatedly rescans those relationships to a fixed point through a stable bounded grace interval, rejects PID reuse on every observation/signal, escalates surviving tracked identities, reaps direct children, closes the PTY, and acknowledges only after every continuously tracked identity is gone. Tests must include grandchildren that change process groups. If macOS cannot reliably observe and terminate a deliberately escaped descendant, stop and obtain a user decision instead of weakening or claiming full-child-tree cleanup.

### 4. Crash-safe restart transition without a second marker file

Do not add `terminal-session-mode.json`. Use existing durable facts:

- The setting `muxy.terminalPersistentSession.enabled` is the desired mode.
- Rust-owned `sessionId` fields in workspace terminal tabs and daemon descriptors are applied-state evidence.
- The process keeps its launch-time mode in memory so a settings toggle can display restart-required state without changing live terminals.

Enabling flow:

1. Confirm the disclosure before persisting desired enabled state.
2. Do not touch current surfaces or PTYs in that process.
3. On the next fresh launch, for each eligible tab without `sessionId`, query the current authenticated Rust daemon by exact immutable owner tuple.
4. If exactly one matching current-protocol descriptor exists, recover its session ID. If none exists, propose a new Rust ID and call idempotent `CreateSession`. More than one match or a launch-fingerprint conflict blocks startup as a protocol invariant error.
5. Persist the returned/recovered session IDs into workspace tabs before allowing renderer materialization.
6. If the app crashes after daemon creation but before workspace persistence, repeat exact-owner recovery on next launch. This is recovery of a session created by the current Rust protocol in the same build mode, not adoption of Swift data, an old daemon, or an arbitrary process.
7. Once a `sessionId` is durably linked to a tab, an absent descriptor means Missing/Ended and is never recreated automatically.
8. After every eligible tab is reconciled and workspace persistence succeeds, materialize Ghostty only for currently visible split panes.

Disabling flow:

1. Show the session count and disclose that confirmed restart will terminate all daemon sessions and descendants.
2. Persist desired disabled state only after confirmation. Do not alter live PTYs.
3. On the next fresh launch, if Rust session IDs or daemon descriptors remain, do not create ordinary terminal surfaces yet.
4. Send `EndAllSessions` and wait for complete cleanup acknowledgement.
5. Clear Rust `sessionId` values and persist workspace state.
6. Materialize ordinary Ghostty terminals only for visible panes.
7. If cleanup or persistence fails, retain IDs, show a blocking recoverable error, and retry on the next launch. Never run ordinary duplicates while daemon descendants may survive.

This derives transition truth without another persistent file and remains idempotent across crashes. `paneSessionID` stays raw, untouched, and semantically ignored. New `sessionId` uses the current uppercase UUID convention.

Before the first main-window render can reconcile workspace terminals, establish `SessionStartupState::{Reconciling, Ready, Blocked(error)}` from app startup. `TerminalSurfaces::reconcile` cannot materialize any workspace surface while Reconciling or Blocked. Enabled startup reaches Ready only after daemon/owner reconciliation and workspace ID persistence succeed. Disabled startup reaches Ready only after complete daemon cleanup and durable ID clearing. Blocked renders a recoverable startup error with Retry and Settings/Quit access and never starts workspace shells. Quick Terminal uses its separate standalone runtime and may bypass only this workspace-terminal barrier. The barrier must be installed before `MainWindow::new` attaches normal workspace reconciliation, because current rendering reconciles terminals immediately (`crates/muxy/src/main.rs:89-135`, `crates/muxy/src/views/window/mod.rs:159-170`, `crates/muxy/src/views/window/render.rs:3-7`).

### 5. Workspace and renderer lifecycle

Add `session_id: Option<SessionId>` to typed terminal tabs and encode it as Rust-owned `sessionId`. Do not read or overwrite `paneSessionID`. Keep browser and extension tabs unaffected.

Persistent Ghostty launches this renderer-side command:

```text
<bundle>/Contents/MacOS/muxy-session attach --socket <path> --session-id <uuid>
```

Ghostty owns the renderer-side terminal and attach-proxy process. The daemon owns the real shell PTY. Dropping a surface ends only the attach proxy; daemon socket EOF means detach, not session exit. Because current Ghostty launch accepts a command string, canonicalize the helper/socket paths, reject NULs, and shell-escape each argv component with the existing neutral terminal escaping helper before joining. Tests must cover spaces and quotes in staged bundle and socket paths; never interpolate raw paths.

Reconciliation has two independent layers:

- Session reconciliation covers every eligible local workspace terminal when persistence is enabled.
- Renderer reconciliation covers visible split panes, explicit selection, and wake requests.

Quick Terminal stays on its separate ordinary path. Remote backing fails persistence eligibility. Hidden restored tabs have no surface until selected. Distinguish these outcomes:

- Attach-proxy or renderer failure: keep workspace/session state and show retryable renderer failure.
- Daemon reports shell exit: close or mark the corresponding attached tab through normal shell-exit behavior.
- Persisted ID missing from daemon: show explicit Missing/Ended state with Start New Terminal and Remove. Never silently rerun a command.

Lifecycle actions:

- **Close Tab and every bulk close mode:** first compute the complete candidate set across single close, close others, close right, root/child closure, workspace removal, and shell-exit reconciliation. Partition ordinary, persistent, browser, and extension tabs. Confirm and end every user-closed persistent session, then mutate/persist tab state only after all required acknowledgements. Shell-exit reconciliation removes only the descriptor-confirmed exited session. A bulk close must never silently turn a persistent tab into a background session. On any required cleanup failure retain the affected candidate state and report it.
- **Send to Background:** clear workspace placement in the daemon, remove the tab from the workspace, and drop its renderer. Keep immutable original owner metadata and session.
- **Focus:** select the existing attached tab.
- **Reattach:** insert a tab with the same session ID into its exact original project/worktree, update current placement, and materialize only if selected and visible. Do not rerun startup command.
- **End:** terminate one session and remove any workspace placement after acknowledgement.
- **End All:** separately confirm with count, terminate all, then reconcile workspace missing/ended state.
- **Quit:** drop renderers/control connections without sending end operations.
- **App-driven project/worktree deletion:** acquire or extend the existing project-operation token, revalidate exact project/worktree identity, gather exact-owner sessions, and include their count in confirmation. After approval, end sessions and verify cleanup, then revalidate the token and owner identity again before invoking filesystem or state deletion. Cleanup failure, cancellation, stale completion, or owner replacement finalizes/releases the operation without mutating project, worktree, workspace, or session placement. Once cleanup has acknowledged, continue through the existing deletion outcome/persistence order; a later workspace persistence failure is surfaced and retried according to existing store behavior, never reported as rollback and never allowed to resurrect ended sessions. Current deletion seams include `crates/muxy/src/views/window/lifecycle.rs:537-577`, `crates/muxy/src/views/window/overlays.rs:2480-2520`, `crates/muxy/src/state.rs:917-946`, `crates/muxy/src/views/window/project_menu.rs`, `crates/muxy/src/views/window/workspace.rs:189`, and `crates/muxy/src/project_operations.rs`.
- **External deletion:** extend `muxy_api::truth` with neutral owner existence facts. Probe exact stored project/worktree paths on startup, app activation, and normal truth refresh. Following a symlink to a directory is Present; `ENOENT`/`ENOTDIR` is Missing; permission, loop, timeout, and other transient I/O are Unknown. Require two current-generation Missing observations before ending exact-owner sessions. Present or Unknown never terminates. Reject stale generations and do not use path prefixes or executable-name matching.

### 6. Idle surface sleeping

Keep the offline policy and state machine in `muxy-terminal`, as required by `PLAN.md:246`. The app supplies visibility, focus, setting, timeout, and session operations. Ghostty and process adapters supply neutral facts.

A local workspace surface is sleep-eligible only when all are true:

- Idle sleeping is enabled.
- Renderer is live.
- Tab is hidden, not merely unfocused.
- It has no focus, input transaction, queued input, resize, or materialization transaction.
- Foreground state is idle.
- No descendant or background process makes ordinary-shell loss unsafe.
- It is not in alternate screen.
- Its activity generation has remained unchanged for the configured timeout.

Unknown sampling fails awake. Visible panes never sleep.

Use explicit states equivalent to Absent, Materializing, Live, SleepPending, Sleeping, Waking, and Failed. Every timer captures tab ID, session/surface identity, and activity generation, then reacquires current state and rechecks every predicate before freeing anything. Increment activity generation on input, output/redraw, focus, visibility, resize, terminal action, and materialization completion. Queue input during wake in a bounded FIFO and reject overflow.

Persistent sleep drops the Ghostty surface and attach proxy while preserving the same daemon session. Wake reattaches the same session, consumes replay, and flushes queued input. Ordinary sleep records last cwd, frees the Ghostty-owned shell/surface only when process loss is safe, accepts documented scrollback loss, and wakes with a fresh shell at that cwd. Neither path reruns a one-shot startup command.

Use existing Ghostty redraw/wakeup signaling for output activity if runtime proof shows it is complete. Only if Phase 5 proves missed output may `ghostty_surface_set_data_callback` be wrapped solely to increment activity generation. It must not become the persistent-session data path.

### 7. Process-tree resource monitor

Create one app-owned, injected `ResourceMonitor`. Status bars subscribe to snapshots. Do not use executable-name matching or global state.

Authenticated roots are:

- Current Muxy PID plus start identity.
- Daemon PID/start identity from `HelloAccepted`.
- Shell/session identities from daemon descriptors.

Do not add a speculative P10 extension-host root API. P10 can extend the root list when that process exists.

The sampler enumerates descendants by process relationships, validates PID start identity, and deduplicates overlapping roots. Portable aggregation computes:

```text
CPU% = delta(user_time + system_time) / elapsed_wall_time * 100
RAM  = sum of resident bytes after PID deduplication
```

Do not divide aggregate CPU by logical CPU count, so multithreaded totals may exceed 100 percent. Sampling failure yields stale or unavailable, never a false zero. The macOS adapter uses validated proc/sysctl APIs. Unsupported targets compile and return unavailable.

Render compact default-on status text such as `CPU 18% · RAM 742 MB`, with live, stale, and unavailable states. A tooltip or popover shows app tree, daemon/session tree, session count, and sample age. Turning the widget off stops its timer and clears previous sample state.

### 8. Session manager, settings, and CLI

Replace the single status-bar trailing slot contract with a neutral trailing group that can contain the P7 Composer footer, resource widget, and session-manager button. Do not put session policy in the reusable status component.

The status-bar session popover contains:

- Workspace Sessions: daemon sessions with workspace placement.
- Background Sessions: daemon sessions without workspace placement.
- Missing/Ended: workspace session IDs with no daemon descriptor.

Rows show title, project/worktree, cwd, and state. Actions are Focus or Reattach as applicable, End, Start New, or Remove. Footer actions are End All Sessions and Terminal Settings. Multiple-session destructive actions show an explicit count. Controls have meaningful accessibility labels and do not use color as the only state signal.

Terminal settings must preserve exact keys/defaults while adding typed runtime consumption and effects:

- `muxy.terminalPersistentSession.enabled`, default false, restart required.
- `muxy.terminalOffline.enabled`, default false.
- `muxy.terminalOffline.idleThresholdSeconds`, default 300 with existing choices.
- `muxy.showResourceUsageInStatusBar`, default true.

Enabling and disabling use the disclosures and confirmation rules in Resolved Design 4. No generic relaunch action is added.

Activate only the already-cataloged P2 commands:

- `list-sessions` returns retained tab-separated columns in this order: `sessionID`, `shellProcessID`, `workingDirectory`, `isAttached`, `title`, `projectID`, `worktreeID`, `tabID` (`Muxy/Services/Socket/SocketCommandHandler.swift:116-129`). Here retained `isAttached` maps to workspace placement, not renderer presence.
- `kill-session <id>` validates the existing argument, sends `EndSession`, and reports success only after full process cleanup. Its missing-argument response stays exactly `error:usage kill-session|sessionID` (`Muxy/Services/Socket/SocketCommandHandler.swift:130-132`).

Use the current P2 response encoder and error conventions. Add no command heads and do not modify the retained CLI file.

## Complete affected-file map

This map gives every planned destination. If implementation proves a listed split unnecessary, keep the owning directory and contract but prefer fewer files. If an unlisted file must change, record why in Progress before editing it.

### Workspace and architecture

- `Cargo.toml`: add `crates/muxy-session`, workspace dependency, and only dependencies proven necessary by adapters.
- `Cargo.lock`: regenerate through Cargo.
- `ARCHITECTURE.md`: update crate count/graph, terminal ownership, session protocol, profile isolation, lifecycle, and target boundaries.
- `PLAN.md`: update only observed P8 implementation status in the final phase. Remove or qualify superseded Swift cross-attach language only after Rust behavior is proven.

### New `muxy-session` crate

- `crates/muxy-session/Cargo.toml`
- `crates/muxy-session/src/lib.rs`
- `crates/muxy-session/src/main.rs`
- `crates/muxy-session/src/client.rs`
- `crates/muxy-session/src/attach.rs`
- `crates/muxy-session/src/daemon/mod.rs`
- `crates/muxy-session/src/daemon/session.rs`
- `crates/muxy-session/src/runtime_paths.rs`
- `crates/muxy-session/src/shell.rs`
- `crates/muxy-session/src/transport/mod.rs`
- `crates/muxy-session/src/transport/unix.rs`
- `crates/muxy-session/src/pty/mod.rs`
- `crates/muxy-session/src/pty/unix.rs`
- `crates/muxy-session/src/process_tree/mod.rs`
- `crates/muxy-session/src/process_tree/macos.rs`
- `crates/muxy-session/src/process_tree/linux.rs`
- `crates/muxy-session/tests/daemon.rs`
- `crates/muxy-session/tests/security.rs`
- `crates/muxy-session/tests/attach.rs`
- `crates/muxy-session/tests/process_cleanup.rs`

### `muxy-proto`

- `crates/muxy-proto/Cargo.toml`
- `crates/muxy-proto/src/lib.rs`
- `crates/muxy-proto/src/session/mod.rs`
- `crates/muxy-proto/src/session/id.rs`
- `crates/muxy-proto/src/session/codec.rs`
- `crates/muxy-proto/src/session/messages.rs`
- `crates/muxy-proto/src/session/replay.rs`
- `crates/muxy-proto/src/session/terminal_stream.rs`
- `crates/muxy-proto/src/session/window_size.rs`

### `muxy-api`

- `crates/muxy-api/src/truth.rs`: add target-neutral Present/Missing/Unknown project/worktree existence facts with current-generation results and no session policy.
- `crates/muxy-api/Cargo.toml`: change only if the neutral existence probe proves a missing dependency.

### `muxy-core`

- `crates/muxy-core/Cargo.toml`
- `crates/muxy-core/src/lib.rs`
- `crates/muxy-core/src/environment.rs`
- `crates/muxy-core/src/workspace/tab.rs`
- `crates/muxy-core/src/workspace_store.rs`
- `crates/muxy-core/src/prefs/mod.rs`
- `crates/muxy-core/src/prefs/settings.rs`
- `crates/muxy-core/src/settings_catalog.rs`
- `crates/muxy-core/src/session/mod.rs`
- `crates/muxy-core/src/session/reconciliation.rs`
- `crates/muxy-core/src/session/transition.rs`
- `crates/muxy-core/src/resources.rs`

### `muxy-terminal` and `ghostty-host`

- `crates/muxy-terminal/Cargo.toml`
- `crates/muxy-terminal/src/lib.rs`
- `crates/muxy-terminal/src/backend.rs`
- `crates/muxy-terminal/src/offline/mod.rs`
- `crates/muxy-terminal/src/offline/policy.rs`
- `crates/muxy-terminal/src/offline/state.rs`
- `crates/muxy-terminal/src/process/mod.rs`
- `crates/muxy-terminal/src/process/macos.rs`
- `crates/muxy-terminal/src/process/unsupported.rs`
- `crates/muxy-terminal/src/ghostty/host_view.rs`
- `crates/ghostty-host/src/surface.rs`

### `muxy` app

- `crates/muxy/Cargo.toml`
- `crates/muxy/src/main.rs`
- `crates/muxy/src/state.rs`
- `crates/muxy/src/command.rs`
- `crates/muxy/src/sessions/mod.rs`
- `crates/muxy/src/sessions/coordinator.rs`
- `crates/muxy/src/sessions/transition.rs`
- `crates/muxy/src/terminal/mod.rs`
- `crates/muxy/src/terminal/surfaces.rs`
- `crates/muxy/src/terminal/idle.rs`
- `crates/muxy/src/terminal/ghostty/mod.rs`
- `crates/muxy/src/terminal/unsupported.rs`
- `crates/muxy/src/resource_monitor/mod.rs`
- `crates/muxy/src/resource_monitor/macos.rs`
- `crates/muxy/src/resource_monitor/unsupported.rs`
- `crates/muxy/src/views/app.rs`
- `crates/muxy/src/views/status_bar.rs`
- `crates/muxy/src/views/session_manager.rs`
- `crates/muxy/src/views/settings/mod.rs`
- `crates/muxy/src/views/settings/categories/terminal.rs`
- `crates/muxy/src/views/window/mod.rs`
- `crates/muxy/src/views/window/terminal.rs`
- `crates/muxy/src/views/window/lifecycle.rs`
- `crates/muxy/src/views/window/commands.rs`
- `crates/muxy/src/views/window/overlays.rs`
- `crates/muxy/src/views/window/render.rs`
- `crates/muxy/src/views/window/view_state.rs`
- `crates/muxy/src/views/window/project_menu.rs`
- `crates/muxy/src/views/window/menu_bar.rs`
- `crates/muxy/src/views/window/workspace.rs`
- `crates/muxy/src/project_operations.rs`
- `crates/muxy/src/socket/runtime.rs`
- `crates/muxy/src/socket/commands/mod.rs`
- `crates/muxy/src/socket/commands/sessions.rs`
- `crates/muxy/src/socket/catalog.rs`: test exact unchanged heads/count only. Do not alter catalog declarations unless current dispatch structure requires moving a P8 marker without changing bytes.

### Build, verification, and documentation

- `scripts/build-app.sh`: build, stage, and sign `Contents/MacOS/muxy-session` with `muxy` from the same profile.
- `scripts/verify-bundle.sh`: require both executables, executable modes, architectures, signatures, exact CLI equality/path, and all five shell resource families.
- `scripts/stage-test-app.sh`: retain helper during staging and verify/resign nested code. Change only if current behavior does not already preserve the helper.
- `scripts/check.sh`: require/run P8 verifier, extend crate/platform boundary gates, forbid P8 ownership leaks, and include affected Linux cross-target checks without weakening prior gates.
- `scripts/verify-p8-terminal-memory.sh`: new safe self-tests, fixtures, staged lifecycle, daemon survival, cleanup, CLI compatibility, and hostile-path verifier.
- `docs/development/session-protocol.md`: protocol, limits, authentication, paths, mismatch, and lifecycle.
- `docs/features/background-sessions.md`: user model, enable/disable restart semantics, Close versus Background, manager, limits, and recovery.
- `docs/features/terminal.md`: idle sleeping, scrollback/process safety, background renderer behavior, and resource widget.
- `docs/features/muxy-cli.md`: activate the two existing session commands without changing wrapper installation.
- `docs/user-guide/settings.md`: exact defaults, restart disclosure, idle timeout, and resource toggle.
- `.github/workflows/**`: unchanged. Rust CI is P15 scope.

## Execution prerequisites and verifier safety

Before Phase 1, verify the required Linux standard library is already installed without changing the repository or entering an offline gate:

```bash
command -v rustup
rustup target list --installed | grep -Fx x86_64-unknown-linux-gnu
test -d "$(rustc --print target-libdir --target x86_64-unknown-linux-gnu)"
cargo metadata --locked --offline --no-deps >/dev/null
```

If these fail, record a missing toolchain prerequisite rather than reporting a source failure. Install the target before resuming offline phase gates. `cargo check` does not link target binaries; if a later native dependency introduces a cross compiler/linker requirement, document and verify that prerequisite before using it and do not hide it inside `scripts/check.sh`.

Before its first process spawn, `scripts/verify-p8-terminal-memory.sh` must install `EXIT`, `INT`, and `TERM` cleanup traps. Cleanup uses only its injected owned test root and recorded PID/start identities, closes the staged app, ends owned sessions, stops attach/helper/daemon processes, verifies shell and descendant death, and removes only safely owned stale socket/lock artifacts. A failed assertion must still run cleanup and fail if residue remains. Provide `--cleanup-only <owned-root>` for interrupted runs, reject production or unowned roots, and record cleanup proof after every phase even when an earlier assertion failed.

## Phases

### Phase 1 - Portable protocol, persistence, policy, and verification skeleton

**Goal:** Establish stable portable contracts with no runtime behavior change.

**Files:**

- Add the `muxy-proto/src/session/**` modules listed in the file map.
- Add `muxy-core/src/session/**` and `muxy-core/src/resources.rs`.
- Update `muxy-core` typed tab/workspace persistence and typed P8 preferences.
- Add `muxy-terminal/src/offline/**` and neutral lifecycle/fact contracts.
- Add `scripts/verify-p8-terminal-memory.sh` with self-test and portable fixtures.
- Update `scripts/check.sh` to require the verifier and enforce the new dependency direction.

**Expected config/manifest changes:**

- Add `muxy-core -> muxy-proto`.
- Add only codec/model dependencies already present in workspace dependencies unless tests prove another is required.
- Add no crate member, executable, socket, runtime file, bundle item, workflow, or live setting effect yet.
- Add Rust-owned optional `sessionId`; leave raw `paneSessionID` unchanged.

**Focused verification:**

- Exact 24-byte header, endianness, allocation-before-length rejection, malformed/truncated/oversized frames, unknown kinds, request correlation, major mismatch, minor negotiation, and protocol limits.
- Complete bounded `CreateSessionRequest`, environment/argv validation, owner-key idempotency, launch-fingerprint conflict, and current-protocol exact-owner recovery.
- Replay bounds, truncation, sequence ordering, UTF-8 and escape boundaries, and alternate-screen generations.
- Session ID validation and uppercase persistence.
- Existing unknown `paneSessionID` survives decode/encode byte shape while new `sessionId` round-trips independently.
- Desired-mode plus persisted-ID transition truth tables cover interrupted enable/disable without a marker file.
- Owner reconciliation uses exact IDs.
- Idle state/timer generation and resource aggregation/dedup math are portable and deterministic.

**Phase gate:**

```bash
cargo test -p muxy-proto --locked --offline session
cargo test -p muxy-core --locked --offline session
cargo test -p muxy-core --locked --offline resources
cargo test -p muxy-terminal --locked --offline offline
scripts/verify-p8-terminal-memory.sh --fixture portable
scripts/verify-p8-terminal-memory.sh --self-test
scripts/check.sh
cargo check -p muxy-proto --all-features --locked --offline --target x86_64-unknown-linux-gnu
cargo check -p muxy-core --all-features --locked --offline --target x86_64-unknown-linux-gnu
cargo check -p muxy-terminal --all-features --locked --offline --target x86_64-unknown-linux-gnu
scripts/build-app.sh debug
scripts/verify-bundle.sh target/debug/Muxy.app debug
staged_app="$(scripts/stage-test-app.sh target/debug/Muxy.app p8-phase-1)"
scripts/verify-p8-terminal-memory.sh --staged debug "$staged_app" phase-1
git diff --check
```

The app must still launch, answer a read-only P2 request through the retained staged CLI, close normally, remove its app socket, and leave no staged process. P8 runtime must remain inactive.

### Phase 2 - Secure daemon and real detach/attach vertical proof

**Goal:** Prove the highest-risk foundation before integrating GUI state: a real daemon-owned shell survives full client disconnect, reattaches with replay, and is completely terminated on demand.

**Files:**

- Add `crates/muxy-session/**` and its integration tests.
- Update root `Cargo.toml`, `Cargo.lock`, and relevant crate manifests.
- Update `muxy-core/src/environment.rs` only for neutral path helpers missing from current policy.
- Update `scripts/build-app.sh`, `scripts/verify-bundle.sh`, `scripts/stage-test-app.sh` only as needed, and P8 verifier daemon fixtures.
- Add initial `docs/development/session-protocol.md` so the implemented wire contract is recorded with the code.

**Expected config/manifest changes:**

- Add ninth workspace member and `muxy-session` workspace dependency.
- Add only proven Unix PTY dependencies. Prefer existing `libc`; do not add an async runtime unless bounded I/O and shutdown cannot be implemented simply with current facilities.
- Stage `Contents/MacOS/muxy-session` from the same profile as `muxy` and sign nested code before signing the app container.
- Add no plist key, entitlement, login item, launch agent, background-service registration, or workflow.

**Focused verification:**

- Same-UID authentication occurs before decode on macOS and compiles with Linux peer credentials.
- Hostile symlinks, non-sockets, wrong ownership/modes, unsafe fallback directories, and stale sockets fail closed.
- Singleton startup races produce one daemon. A live socket is never unlinked.
- Debug and release locks/sockets/logs cannot collide.
- A real shell receives input and initial/rapid resize, emits ordered output, survives complete attach disconnect, and replays to a new attach. Stale attach generations cannot resize replacements.
- New attach replaces a stale active renderer deterministically. No multi-client fan-out exists.
- A non-reading renderer hits the output cap, disconnects without blocking PTY drain or EndSession, and can later reconnect from bounded replay.
- Bounds cover 64 control connections, frame/chunk/input/output/replay limits, and mismatch behavior.
- zsh, bash, fish, elvish, and nushell transformation vectors preserve existing environment semantics.
- EndSession fixed-point supervision tracks grandchildren and changed process groups, rejects PID reuse, and acknowledges only after every tracked identity is gone and direct children are reaped.
- No-session/no-connection daemon exits after its bounded idle timeout.

**Phase gate:**

```bash
cargo test -p muxy-session --all-targets --all-features --locked --offline
scripts/verify-p8-terminal-memory.sh --fixture protocol
scripts/verify-p8-terminal-memory.sh --fixture security
scripts/verify-p8-terminal-memory.sh --fixture daemon-detach-attach
scripts/verify-p8-terminal-memory.sh --fixture process-cleanup
scripts/verify-p8-terminal-memory.sh --fixture shell-integration
scripts/check.sh
cargo check -p muxy-session --all-features --locked --offline --target x86_64-unknown-linux-gnu
scripts/build-app.sh debug
scripts/verify-bundle.sh target/debug/Muxy.app debug
staged_app="$(scripts/stage-test-app.sh target/debug/Muxy.app p8-phase-2)"
scripts/verify-p8-terminal-memory.sh --staged debug "$staged_app" phase-2
git diff --check
```

The staged proof must create a sentinel shell, disconnect the attach completely, prove the shell PID/start identity still exists, reattach and observe replay plus a new command, close the app normally, prove the daemon/session still exists, then explicitly end the session and prove daemon, shell, and descendant cleanup. The verifier must own an injected test root and refuse production paths.

### Phase 3 - Restart-only mode transition and Ghostty attachment

**Goal:** Integrate daemon ownership with every eligible local workspace tab while preserving restart-only toggles and visible-only renderer creation.

**Files:**

- Add `muxy/src/sessions/**` and wire one coordinator through `main.rs`, `state.rs`, and window state using the sole public `muxy-session` client facade.
- Establish the startup barrier before first workspace reconciliation in `main.rs`, `views/window/mod.rs`, `view_state.rs`, `render.rs`, and `terminal/surfaces.rs`; render Retry/Settings/Quit while blocked.
- Update typed settings effects and terminal settings UI.
- Update `terminal/ghostty/mod.rs`, `terminal/surfaces.rs`, terminal backend contracts, and unsupported target.
- Update `views/window/terminal.rs` and lifecycle startup/error presentation.
- Extend P8 verifier with enable, interrupted-transition, visible-only, Quick Terminal exclusion, and disable fixtures.

**Expected config/manifest changes:**

- Add `muxy -> muxy-session` for its narrow public client facade. Keep daemon/session-table/PTY internals private and do not add an app transport implementation.
- Create only existing profile-specific session directory/socket/lock/log at runtime.
- Do not add `terminal-session-mode.json` or another preference key.
- Preserve exact existing settings keys/defaults and unknown-key behavior.

**Focused verification:**

- Toggle changes desired mode and restart-required UI but has no live PTY/surface effect.
- Reconciling and Blocked startup states prevent every workspace surface spawn; Ready is reached only after durable enable or disable reconciliation. Quick Terminal alone bypasses that barrier.
- Enabling never adopts existing ordinary PIDs. It recovers only current-protocol exact-owner descriptors, creates missing daemon sessions idempotently, and durably links all eligible local workspace tabs before renderers.
- Hidden tabs receive session IDs and daemon shells but no Ghostty surfaces. Selecting one attaches it.
- Only visible split panes materialize on relaunch.
- Quick Terminal remains ordinary and remote terminals remain excluded.
- Canonical, NUL-free, per-argument shell-escaped helper commands work when staged bundle/socket paths contain spaces and quotes.
- Attach-proxy exit does not close the backing workspace tab. Daemon shell exit does.
- Missing daemon session shows explicit Missing/Ended state and never silently reruns startup command.
- Confirmed disable ends all descendants before IDs are cleared and ordinary visible surfaces start.
- Failure during enable/disable is idempotently retryable and never creates duplicate ordinary terminals.
- Debug/release app and helper resolve isolated paths and same-profile sibling binaries.

**Phase gate:**

```bash
cargo test -p muxy-core --locked --offline session
cargo test -p muxy-terminal --locked --offline offline
cargo test -p muxy --locked --offline sessions
cargo test -p muxy --locked --offline terminal
scripts/verify-p8-terminal-memory.sh --fixture transitions
scripts/verify-p8-terminal-memory.sh --fixture renderer-attachment
scripts/verify-p8-terminal-memory.sh --fixture exclusions
scripts/check.sh
cargo check -p muxy-session --all-features --locked --offline --target x86_64-unknown-linux-gnu
cargo check -p muxy --all-features --locked --offline --target x86_64-unknown-linux-gnu
scripts/build-app.sh debug
scripts/verify-bundle.sh target/debug/Muxy.app debug
staged_app="$(scripts/stage-test-app.sh target/debug/Muxy.app p8-phase-3)"
scripts/verify-p8-terminal-memory.sh --staged debug "$staged_app" phase-3
git diff --check
```

The staged app must demonstrate restart-only enable, hidden surface absence, selected-tab attachment, normal Quit with daemon survival, reopen to the same session, confirmed disable cleanup, normal close, and zero residue after explicit cleanup.

### Phase 4 - Full workspace and background-session lifecycle

**Goal:** Complete user-visible ownership semantics before adding memory policy or status UI.

**Files:**

- Update session coordinator and daemon control messages as required by lifecycle.
- Update `views/window/terminal.rs`, `commands.rs`, `lifecycle.rs`, `overlays.rs`, `project_menu.rs`, `menu_bar.rs`, `workspace.rs`, `state.rs`, and `project_operations.rs`.
- Extend `muxy-api/src/truth.rs` with neutral current-generation project/worktree existence facts and wire startup/activation/refresh probes in app lifecycle.
- Add command/action paths for Send to Background and Start New Terminal.
- Add non-popover state models needed by later session-manager UI.
- Extend P8 verifier with close, background, reattach, deletion, orphan, quit, crash/missing, and startup-command fixtures.

**Expected config/manifest changes:**

- No new executable, protocol framing, setting key, storage file, bundle resource, CLI head, or workflow.
- Protocol minor may increase only if Phase 2 lacks a lifecycle message. The old minor must fail or negotiate according to the documented table.

**Focused verification:**

- Single Close Tab and every `CloseMode` compute candidates first, partition mixed ordinary/persistent/browser/extension tabs, end user-closed persistent sessions, and mutate/persist only after all required acknowledgements. None implicitly becomes Background.
- Send to Background removes placement and renderer without ending the session.
- Focus selects an existing workspace tab. Reattach restores the exact original project/worktree and same session ID.
- Reattach and Start New never rerun an old one-shot startup command.
- App-driven project/worktree deletion holds/revalidates the operation token around counted confirmation and exact-owner cleanup, then mutates only after a second identity check. Cancellation, daemon failure, stale completion, and owner replacement leave project/worktree/workspace state unchanged. A later persistence failure is explicit/retryable and cannot resurrect ended sessions or be reported as rollback.
- External removal reconciliation uses two current-generation Missing existence facts. Present, Unknown, symlink loops, permission errors, and stale probes never terminate; confirmed exact orphans do.
- Normal Quit and app crash detach only. End All is separately confirmed.
- Attached shell exit, detached shell exit, daemon crash, attach failure, and missing ID have distinct outcomes.
- Failures retain recoverable workspace state instead of silently dropping tabs or duplicating processes.

**Phase gate:**

```bash
cargo test -p muxy-api --locked --offline truth
cargo test -p muxy-core --locked --offline reconciliation
cargo test -p muxy-session --locked --offline lifecycle
cargo test -p muxy --locked --offline session_lifecycle
scripts/verify-p8-terminal-memory.sh --fixture close-background-reattach
scripts/verify-p8-terminal-memory.sh --fixture all-close-modes
scripts/verify-p8-terminal-memory.sh --fixture owner-deletion-transaction
scripts/verify-p8-terminal-memory.sh --fixture external-owner-truth
scripts/verify-p8-terminal-memory.sh --fixture quit-crash-missing
scripts/verify-p8-terminal-memory.sh --fixture startup-command-once
scripts/check.sh
cargo check -p muxy-api --all-features --locked --offline --target x86_64-unknown-linux-gnu
cargo check -p muxy-session --all-features --locked --offline --target x86_64-unknown-linux-gnu
cargo check -p muxy --all-features --locked --offline --target x86_64-unknown-linux-gnu
scripts/build-app.sh debug
scripts/verify-bundle.sh target/debug/Muxy.app debug
staged_app="$(scripts/stage-test-app.sh target/debug/Muxy.app p8-phase-4)"
scripts/verify-p8-terminal-memory.sh --staged debug "$staged_app" phase-4
git diff --check
```

The staged proof must leave a daemon shell running through normal app close, reopen and reattach it, exercise Background and exact-owner deletion, then use an explicit destructive action and prove all descendants and sockets are cleaned.

### Phase 5 - Idle Ghostty surface sleeping and wake

**Goal:** Free eligible hidden renderer surfaces without confusing renderer lifetime with persistent shell lifetime.

**Files:**

- Complete `muxy-terminal/src/offline/**`, process adapters, and backend lifecycle/activity contracts.
- Update Ghostty host view and `ghostty-host/src/surface.rs` for neutral activity/cell facts.
- Add app timer/orchestration in `muxy/src/terminal/idle.rs` and `terminal/surfaces.rs`.
- Update terminal settings runtime effect for idle enable/timeout.
- Add the data callback wrapper only if runtime proof requires it.
- Extend P8 verifier with policy, race, wake queue, ordinary, persistent, process safety, and output activity cases.

**Expected config/manifest changes:**

- No new key. Preserve default false and existing timeout values.
- Add target-specific dependencies only if real macOS process inspection cannot use existing `libc`/Ghostty seams.
- No daemon protocol change unless persistent foreground/process facts are demonstrably missing.

**Focused verification:**

- Only hidden tabs sleep. Visible but unfocused panes stay live.
- Stale timers cannot free a replaced/reselected surface.
- Input, output, focus, visibility, resize, action, and materialization increment activity.
- Input and resize during wake are ordered and bounded.
- Alternate screen, foreground child, background descendant, unknown sampling, and transaction activity fail awake.
- Persistent sleep removes Ghostty/attach proxy but preserves daemon shell PID/start identity and session ID. Wake reattaches with replay.
- Ordinary sleep occurs only when process loss is safe, restores cwd, documents scrollback loss, and does not rerun startup command.
- Runtime output proof determines whether the data callback is needed. If added, tests prove it only signals activity.

**Phase gate:**

```bash
cargo test -p muxy-terminal --all-targets --all-features --locked --offline offline
cargo test -p muxy --locked --offline terminal_idle
scripts/verify-p8-terminal-memory.sh --fixture idle-policy
scripts/verify-p8-terminal-memory.sh --fixture idle-races
scripts/verify-p8-terminal-memory.sh --fixture idle-process-safety
scripts/verify-p8-terminal-memory.sh --fixture persistent-sleep-wake
scripts/verify-p8-terminal-memory.sh --fixture ordinary-sleep-wake
scripts/check.sh
cargo check -p muxy-terminal --all-features --locked --offline --target x86_64-unknown-linux-gnu
cargo check -p muxy --all-features --locked --offline --target x86_64-unknown-linux-gnu
scripts/build-app.sh debug
scripts/verify-bundle.sh target/debug/Muxy.app debug
staged_app="$(scripts/stage-test-app.sh target/debug/Muxy.app p8-phase-5)"
scripts/verify-p8-terminal-memory.sh --staged debug "$staged_app" phase-5
git diff --check
```

The staged app must prove a hidden persistent surface disappears while its daemon shell survives, selecting the tab recreates the renderer with replay, an active child prevents ordinary sleep, and explicit cleanup leaves no process.

### Phase 6 - Process-tree resource monitor and status item

**Goal:** Deliver identity-safe CPU/RAM reporting without timer or status-slot conflicts.

**Files:**

- Complete `muxy-core/src/resources.rs` portable identities/math.
- Add `muxy/src/resource_monitor/**` and inject one monitor into app/window state.
- Update `views/status_bar.rs` and `views/app.rs` with a neutral trailing group and resource item.
- Update typed resource setting effect so disabling stops sampling and clears state.
- Extend P8 verifier with synthetic aggregation and real process-tree fixtures.

**Expected config/manifest changes:**

- Preserve `muxy.showResourceUsageInStatusBar = true`.
- Add no storage file, daemon command, executable, or workflow.
- Platform proc/sysctl code remains macOS-gated; unsupported targets compile unavailable behavior.

**Focused verification:**

- CPU delta uses process time over elapsed wall time and permits totals above 100 percent.
- Resident memory sums after deduplicating overlapping app, daemon, shell, and descendant roots.
- PID start mismatch prevents reuse contamination.
- Real app, daemon, session shell, and grandchild process fixtures all contribute once.
- Sampling failures transition to stale/unavailable, never false zero.
- Turning the widget off stops timers and drops prior deltas. Turning it on starts with a fresh baseline.
- Resource item coexists with Composer footer and does not claim the future session-manager slot exclusively.

**Phase gate:**

```bash
cargo test -p muxy-core --locked --offline resources
cargo test -p muxy --locked --offline resource_monitor
scripts/verify-p8-terminal-memory.sh --fixture resource-math
scripts/verify-p8-terminal-memory.sh --fixture resource-process-tree
scripts/check.sh
cargo check -p muxy-core --all-features --locked --offline --target x86_64-unknown-linux-gnu
cargo check -p muxy --all-features --locked --offline --target x86_64-unknown-linux-gnu
scripts/build-app.sh debug
scripts/verify-bundle.sh target/debug/Muxy.app debug
staged_app="$(scripts/stage-test-app.sh target/debug/Muxy.app p8-phase-6)"
scripts/verify-p8-terminal-memory.sh --staged debug "$staged_app" phase-6
git diff --check
```

The staged proof must spawn an owned daemon session and descendant load process, observe nonzero resource snapshots and session count, disable the widget and prove sampling stops, close normally, then explicitly clean all owned processes.

### Phase 7 - Session-manager popover and P2 CLI activation

**Goal:** Expose complete session control through the approved status-bar UX and existing CLI heads without changing P2 compatibility.

**Files:**

- Add `views/session_manager.rs` and status composition/actions.
- Complete settings access and destructive confirmations.
- Add `socket/commands/sessions.rs`, export it, and wire existing runtime dispatch.
- Update `socket/catalog.rs` tests only as necessary to pin unchanged head declarations/count.
- Update `docs/features/muxy-cli.md` only after exact bytes pass.
- Extend P8 verifier with popover model/action and exact CLI fixtures.

**Expected config/manifest changes:**

- No new CLI command head, wrapper byte, bundle path, framing, setting, executable, or workflow.
- No status-bar persistence key.

**Focused verification:**

- Attached means workspace placement, even when a hidden tab has no renderer.
- Popover sections and Focus, Reattach, End, End All, Start New, Remove, and Settings actions map to coordinator operations.
- Exact-owner invalidation disables Reattach and orphan cleanup follows.
- Counted destructive confirmations and accessibility labels are deterministic.
- Composer footer, resource widget, and session button coexist at narrow widths.
- `list-sessions` emits exact tab-separated column order and current P2 terminator/error convention.
- `kill-session` missing argument is exactly `error:usage kill-session|sessionID`; invalid UUID, not found, mismatch, daemon unavailable, and success after cleanup are pinned.
- Accepted-head count, retained CLI source bytes, and legacy path remain unchanged.

**Phase gate:**

```bash
cargo test -p muxy --locked --offline session_manager
cargo test -p muxy --locked --offline socket
scripts/verify-p8-terminal-memory.sh --fixture session-manager
scripts/verify-p8-terminal-memory.sh --fixture cli-sessions
scripts/verify-p8-terminal-memory.sh --fixture status-trailing-group
scripts/check.sh
cargo check -p muxy --all-features --locked --offline --target x86_64-unknown-linux-gnu
scripts/build-app.sh debug
scripts/verify-bundle.sh target/debug/Muxy.app debug
staged_app="$(scripts/stage-test-app.sh target/debug/Muxy.app p8-phase-7)"
scripts/verify-p8-terminal-memory.sh --staged debug "$staged_app" phase-7
cmp -s Muxy/Resources/scripts/muxy-cli target/debug/Muxy.app/Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli
git diff --check
```

The staged proof must exercise the retained CLI through its legacy bundle path against the injected app socket, confirm a listed session survives app close, reopen, kill it through CLI, and prove cleanup. It must also exercise manager actions through production coordinator logic, not a test-only duplicate.

### Phase 8 - Hardening, documentation, full profile matrix, and roadmap closure

**Goal:** Lock security, boundaries, bundle inventory, user documentation, and complete debug/release evidence without weakening prior gates.

**Files:**

- Complete `scripts/verify-p8-terminal-memory.sh` final matrix and hostile self-tests.
- Strengthen `scripts/check.sh` boundary and exact-contract checks.
- Complete bundle build/verifier nested-code and five-shell inventory.
- Update `ARCHITECTURE.md`, P8 docs, `docs/features/terminal.md`, settings docs, CLI docs, and observed `PLAN.md` status.
- Update this plan's Progress section.

**Expected config/manifest changes:**

- All manifest additions are documented and mechanically bounded.
- Both profiles contain exactly the expected app/helper executables and current resource/CLI inventory.
- No CI workflow change. No launch agent, login item, daemon installer, or generic relaunch mechanism.

**Focused verification:**

- Static gates pin dependency direction, protocol ownership, idle-policy ownership, no marker file, no `paneSessionID` adoption, exact keys/defaults, one renderer attach, and unchanged P2 contracts.
- Verifier self-tests reject production roots, foreign ownership, unsafe modes, symlinks, stale owner markers, live-socket unlink, wrong profile helper, and cleanup outside owned roots.
- Debug and release staged identities remain isolated from each other and production.
- Full daemon stress covers repeated attach/detach, replay truncation, input/output bounds, startup races, shell exit, full process-tree termination, and daemon idle exit.
- Documentation distinguishes observed automated behavior from manual/native acceptance.

**Phase gate:**

```bash
cargo test --workspace --all-targets --all-features --locked --offline
scripts/verify-p5-notifications.sh --self-test
scripts/verify-p6-quick-terminal.sh --self-test
scripts/verify-p7-composer.sh --self-test
scripts/verify-p8-terminal-memory.sh --self-test
scripts/verify-p8-terminal-memory.sh --fixture all
scripts/check.sh
cargo check -p muxy-proto --all-features --locked --offline --target x86_64-unknown-linux-gnu
cargo check -p muxy-api --all-features --locked --offline --target x86_64-unknown-linux-gnu
cargo check -p muxy-core --all-features --locked --offline --target x86_64-unknown-linux-gnu
cargo check -p muxy-terminal --all-features --locked --offline --target x86_64-unknown-linux-gnu
cargo check -p muxy-session --all-features --locked --offline --target x86_64-unknown-linux-gnu
cargo check -p muxy --all-features --locked --offline --target x86_64-unknown-linux-gnu

scripts/build-app.sh debug
scripts/verify-bundle.sh target/debug/Muxy.app debug
debug_app="$(scripts/stage-test-app.sh target/debug/Muxy.app p8-final-debug)"
scripts/verify-p8-terminal-memory.sh --staged debug "$debug_app" final-debug

scripts/build-app.sh release
scripts/verify-bundle.sh target/release/Muxy.app release
release_app="$(scripts/stage-test-app.sh target/release/Muxy.app p8-final-release)"
scripts/verify-p8-terminal-memory.sh --staged release "$release_app" final-release

cmp -s Muxy/Resources/scripts/muxy-cli target/debug/Muxy.app/Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli
cmp -s Muxy/Resources/scripts/muxy-cli target/release/Muxy.app/Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli
codesign --verify --deep --strict target/debug/Muxy.app
codesign --verify --deep --strict target/release/Muxy.app
git diff --check
```

Both staged profiles must complete enable, daemon-shell creation, hidden surface absence, renderer attach, app close with session survival, reopen/reattach, background/manager/CLI actions, idle sleep/wake, resource observation, confirmed disable, normal close, and zero owned process/socket residue. The release run must not touch debug state and vice versa.

## Definition of done

### Background-session behavior

- Optional persistence defaults off and affects only local workspace terminal tabs.
- Quick Terminal and remote terminals remain ordinary and excluded.
- Enabling/disabling is restart-only and never adopts or converts live PTYs.
- Startup reconciliation blocks all workspace renderer materialization until enabled-session linkage or disabled cleanup and persistence is complete; blocked failures cannot spawn duplicate ordinary shells.
- Fresh enabled launch recovers only exact-owner sessions created by the current Rust protocol, daemon-backs all eligible tabs idempotently, durably links IDs, and materializes only visible split panes.
- A session survives complete GUI Quit and reattaches to the same daemon-owned shell.
- Detached sessions have no Ghostty surface or attach proxy. Hidden tabs restored without a renderer remain surface-free until selected; after materialization, hiding retains the occluded renderer unless the separate idle policy sleeps it.
- One active renderer attachment per session is deterministic and bounded.
- Single and bulk close modes end every user-closed persistent backing tree before tab mutation and never implicitly background one. Send to Background is the only tab-removal action that preserves it without workspace placement.
- Reattach returns to exact original project/worktree and never reruns startup command.
- Confirmed disable ends all daemon sessions/descendants before clearing IDs or starting ordinary terminals.
- App-driven deletion holds and revalidates its operation/owner identity around exact-owner cleanup before mutation. External disappearance ends exact orphans only after two current Missing facts; Unknown fails safe.
- Attach failure, backing shell exit, missing session, app crash, and daemon crash are not conflated.
- Ordinary Quit never sends EndSession or EndAllSessions.

### Idle and resource behavior

- Idle sleeping is separately default off and preserves existing timeout choices.
- Visible panes never sleep. Unknown safety facts fail awake.
- Persistent sleep removes only renderer/client; ordinary sleep occurs only when losing its shell is safe.
- Wake is race-safe, input bounded/ordered, cwd restored for ordinary terminals, and one-shot startup commands never rerun.
- Scrollback loss for ordinary sleeping is documented. Persistent replay is bounded and ordered.
- Resource widget remains default on, includes app, daemon, shell, and descendants, deduplicates PIDs, validates start identity, and has live/stale/unavailable states.
- Disabling resource display stops sampling and clears delta state.

### Protocol, security, and cleanup

- Protocol is versioned, bounded, same-user authenticated, and explicitly rejects mismatch/malformed input.
- Swift frames, old daemons, old sessions, and `paneSessionID` are never adopted.
- Runtime leaf, socket, lock, and log follow ownership/mode/no-follow rules and stable debug/release names.
- A live daemon socket is never unlinked. Singleton startup and stale recovery are race-safe.
- Session supervision continuously tracks shell-session, process-group, TTY, ancestry, and PID/start identities; termination rescans to a fixed point, escalates boundedly, reaps direct children, and acknowledges only after every tracked identity is gone.
- Debug, release, and staged test roots, daemons, sockets, and helpers are isolated.
- App/helper bundles contain correct architectures, executable modes, nested signatures, and all five shell integrations.

### CLI, UX, and compatibility

- Session manager is a status-bar popover with approved sections/actions and accessible counted destructive controls.
- Composer footer, resource item, and session button coexist through a composed trailing group.
- Exact P2 framing, accepted heads/count, retained CLI source bytes, and legacy bundle path remain unchanged.
- `list-sessions` and `kill-session` use exact retained shapes and cleanup acknowledgement ordering.
- No unrelated Stage B, P10, P15 CI, updater, signing-distribution, or relaunch feature enters P8.

### Quality floor checked in every phase

- `scripts/check.sh`, prior verifiers, P8 verifier, lint configuration, bundle checks, and architecture gates are updated for the new structure, never deleted or weakened.
- No new `#[allow]`, `#[expect]`, Cargo lint weakening, code comment, `static mut`, global mutable state, or thread-local policy bypass is introduced.
- `muxy-proto` remains independent. `muxy-api`, `muxy-core`, `muxy-terminal`, `muxy-session`, app, and Ghostty code follow the documented dependency direction. The app uses the sole public `muxy-session` client facade and has no duplicate transport.
- Shared protocol, API, core, terminal, and UI locations carry no caller-specific project, window, route, Composer, or future-extension policy.
- Idle/offline policy remains in `muxy-terminal`; app policy remains in `muxy`; FFI remains in `ghostty-host`.
- No second session-mode marker file is introduced. Restart transition is derived idempotently from desired setting, Rust session IDs, and daemon truth.
- No new test seam can target production paths or bypass same-user/path validation.
- The P8 verifier installs signal/exit cleanup traps before spawning, supports owned-root cleanup-only recovery, records PID/start identities, and fails on residue even after an earlier assertion fails.
- Every queue, task, timer, socket, renderer, PTY, child, and monitor has deterministic stop/drop behavior and rejects stale completions by identity/generation.
- Every phase is a safe stopping point with focused proof, full gates, verified bundle, staged launch, normal close, daemon behavior proof appropriate to that phase, and explicit cleanup.
- Linux cross-target checks cover every affected portable/headless package. Linux-host launch remains unclaimed P15 scope.

### Documentation and evidence

- Architecture, protocol, terminal, settings, background-session, and CLI docs match only implemented behavior.
- Progress records exact commands, outputs, failures, process identities, and unverified native behavior after each phase.
- A fresh post-implementation audit is completed against all locked decisions and this Definition of done. Material concerns become numbered hardening tasks, one concern per task, each kept green before the next.

## Verification

### Full automated proof

Use the Phase 8 command block as the minimum complete automated proof. Also inspect these load-bearing seams directly after the final diff:

- `crates/muxy-proto/src/session/codec.rs` and `messages.rs` for pre-allocation limits, negotiation, complete bounded launch request, owner idempotency, and protocol isolation.
- `crates/muxy-session/src/client.rs`, `runtime_paths.rs`, and `transport/unix.rs` for one shared client facade, descriptor-relative no-follow behavior, singleton locking, stale socket rules, and peer authentication.
- `crates/muxy-session/src/daemon/session.rs`, PTY, and process-tree modules for detach survival, replay/live ordering, PID identity, reaping, and cleanup acknowledgement.
- `crates/muxy-core/src/workspace/tab.rs`, `workspace_store.rs`, and session transition modules for `sessionId`, raw `paneSessionID` preservation, and crash-idempotent restart transitions.
- `crates/muxy-terminal/src/offline/**` for hidden-only eligibility, fail-awake policy, timer generation, and wake queue bounds.
- `crates/muxy/src/sessions/coordinator.rs`, startup/view state, `terminal/surfaces.rs`, and `views/window/terminal.rs` for the pre-render startup barrier, all-tabs session truth versus visible-only renderer truth, and exit distinction.
- `crates/muxy-api/src/truth.rs` and project/worktree deletion seams for fail-safe owner existence and token-revalidated pre-deletion exact-owner cleanup.
- `crates/muxy/src/resource_monitor/**` and `muxy-core/src/resources.rs` for authenticated roots, PID dedup, elapsed-time math, and timer shutdown.
- `crates/muxy/src/views/status_bar.rs`, `views/session_manager.rs`, and `views/app.rs` for neutral composition and approved UX.
- P2 socket dispatch plus `scripts/build-app.sh`, `verify-bundle.sh`, `check.sh`, and P8 verifier for stale paths or weakened gates.

### Manual native acceptance

Prepare a staged debug app through the safe verifier:

```bash
scripts/build-app.sh debug
scripts/verify-bundle.sh target/debug/Muxy.app debug
manual_app="$(scripts/stage-test-app.sh target/debug/Muxy.app p8-manual-debug)"
scripts/verify-p8-terminal-memory.sh --manual debug "$manual_app"
```

The verifier must print exact injected app-support/session/socket/log paths, daemon cleanup commands restricted to that owned root, and normal-close instructions. Record each result separately:

1. Persistence setting is off by default. Enabling explains replacement after Quit/reopen and does not affect current terminals.
2. After Quit/reopen, every local workspace tab has a daemon session, but only visible split panes have renderer surfaces.
3. A hidden tab's shell continues producing output without a Ghostty surface; selecting it attaches and displays bounded replay.
4. Quick Terminal remains ordinary. Any remote terminal remains excluded.
5. Single close, close others, close right, root/child closure, and workspace removal with mixed tab kinds confirm as required, end every persistent candidate before mutation, and never silently background one.
6. Send to Background removes the tab while the process remains. Popover Reattach restores it to the original project/worktree.
7. Focus, End, End All, Start New, Remove, and Settings actions have correct states, labels, keyboard behavior, and confirmations.
8. Ordinary Quit leaves sessions running with no Ghostty surface. Reopen returns to the same shell PID/start identity.
9. Project/worktree deletion reports the affected count and does not continue if session cleanup fails.
10. External owner removal reconciles the orphan without affecting unrelated sessions.
11. Killing only an attach proxy does not close the tab or daemon shell. A real daemon shell exit follows shell-exit behavior.
12. A missing daemon session displays Missing/Ended and never silently reruns a one-shot command.
13. Idle sleeping off leaves hidden surfaces alone. With it on, an eligible hidden surface disappears after each existing timeout choice and wakes on selection.
14. Alternate-screen applications, active foreground children, background descendants, current input, and unknown process state prevent ordinary sleeping.
15. Persistent wake preserves the daemon shell and replay. Ordinary wake restores cwd, loses documented scrollback, and does not rerun startup command.
16. Resource status is visible by default, changes under app and session load, includes descendants, and presents stale/unavailable honestly.
17. Turning resource display off stops updates and leaves Composer footer/session button usable.
18. Status bar remains usable at narrow widths, light/dark themes, and scaled UI. Popover does not rely on color alone and VoiceOver labels identify actions and owners.
19. `list-sessions` and `kill-session` work through the staged retained CLI path with exact output/error shape.
20. Disabling persistence warns with count. After confirmed Quit/reopen all daemon descendants are gone before ordinary visible terminals open.
21. Debug and release apps can coexist without seeing or killing each other's sessions.
22. Final normal close plus explicit End All/disable cleanup leaves no staged app, attach proxy, daemon, shell, descendant, socket, lock holder, or monitor task.

Unobserved items remain unverified and cannot be used to mark P8 fully accepted.

### Post-implementation audit and hardening

After all eight phases complete, start a fresh read-only audit agent with this plan, the final diff, locked decisions, and Definition of done. Require facts only with file:line evidence. It must cover:

- Every locked decision and every affected-file destination.
- Protocol and crate dependency direction.
- Runtime-path, singleton, peer-authentication, PID-reuse, and cleanup seams.
- Desired/applied transition crash windows and duplicate-process prevention.
- Renderer versus shell exit and hidden-surface ownership.
- Close, Background, Reattach, deletion, orphan, Quit, End All, and missing-session semantics.
- Idle timer/input/output/process races and startup-command non-reexecution.
- Resource root authentication, descendant enumeration, deduplication, and timer shutdown.
- P2 bytes/path/framing/catalog stability.
- Bundle/signing/resource and verifier/check-script stale paths.
- Documentation claims versus observed evidence.

Judge findings independently. Convert material findings into a small numbered hardening list with one concern and concrete paths per task. Execute one task at a time, rerun its focused tests, `scripts/check.sh`, bundle/staged lifecycle, and update Progress before the next task. Independently run static checks and read changed seams after an agent reports completion. State plainly which builds or runtime behavior could not be verified and how the user can verify them.

## Known unverified facts

1. Planning did not run Cargo, Swift, shell verifier, bundle, launch, daemon, process, or Linux cross-target commands. The current green state is unknown.
2. Ghostty attach-proxy stdin/stdout transparency, renderer-side PTY behavior, and resize propagation are not runtime-proven.
3. Existing Ghostty redraw/wakeup signals may not account for every output activity event. Phase 5 must prove this before adding a data callback wrapper.
4. Exact Rust API/dependency choices for PTY creation, macOS `LOCAL_PEERCRED`, Linux `SO_PEERCRED`, macOS process start identity, proc resource data, and sysctl process records remain implementation decisions.
5. Exact child-helper nested signing and future distribution entitlement requirements are not proven. P8 proves current ad-hoc debug/release bundles only; notarized distribution remains later release work.
6. Real aggregate macOS CPU semantics and process descendant coverage must be validated against owned test processes before UI values are considered final.
7. Daemon crash survival of its PTY descendants is not promised. P8 guarantees GUI detach survival, not daemon-failure persistence.
8. Bounded replay may omit older output and ordinary idle sleep loses scrollback. Both limitations must be disclosed rather than hidden.
9. Linux-host launch behavior is outside P8. Only cross-target compilation and fail-safe unsupported adapters are required.
10. The retained Swift filtered tests may not compile in the current environment and are parity evidence, not a mandatory Rust implementation dependency. Any run failure must be recorded honestly.
11. No simultaneous multi-renderer UX is approved. If implementation proves more than one renderer is currently required, stop and obtain a user decision rather than adding fan-out implicitly.
12. No deviation from the locked P8 decisions is knowingly required. If implementation proves one unavoidable, stop, record file:line evidence in Progress, and obtain user approval before proceeding.

## Progress

### Phase 1 - Portable protocol, persistence, policy, and verification skeleton

**Completed:** 2026-08-30 22:00:54 UTC

**Implemented:**

- Added portable versioned session protocol models, strict uppercase `SessionId`, bounded 24-byte frame codec, complete bounded creation contract, replay buffering, terminal-stream parsing, and window-size validation in `muxy-proto`.
- Added desired/applied startup transitions, exact-owner reconciliation, typed workspace `sessionId` persistence, malformed-link blocking evidence, and identity-safe resource aggregation in `muxy-core`. Raw `paneSessionID` and non-terminal `sessionId` values remain untouched.
- Added exact P8 preference keys/defaults and timeout choices with no live setting effect.
- Added portable idle eligibility, lifecycle generation, timer-time policy revalidation, bounded wake input, and neutral backend facts in `muxy-terminal`.
- Added the executable P8 verifier, portable fixture, hostile cleanup self-test, staged Phase 1 lifecycle proof, and `scripts/check.sh` ownership/boundary gates. P5, P6, and P7 scope exceptions permit only `muxy-proto/src/session/**` and `muxy-proto/src/lib.rs`.
- Added only `muxy-core -> muxy-proto`. No daemon crate, executable, socket behavior, runtime marker, bundle item, workflow, or P8 app runtime integration was added.

**Prerequisites:**

```text
command -v rustup
rustup target list --installed | grep -Fx x86_64-unknown-linux-gnu
test -d "$(rustc --print target-libdir --target x86_64-unknown-linux-gnu)"
cargo metadata --locked --offline --no-deps >/dev/null
```

All passed. `rustup` resolved to `/Users/saeed/.cargo/bin/rustup`, the Linux target and target library directory were present, and Cargo metadata succeeded with only its format-version warning.

**Observed failures and fixes:**

1. The first grouped core test attempt failed because `Cargo.lock` needed updating after adding `muxy-core -> muxy-proto`. `cargo check -p muxy-core --offline` updated the lockfile and then exposed an unused import and an untyped `HashMap`; both were fixed.
2. Two existing `muxy-proto` server tests failed in a restricted filesystem sandbox with `SocketBind` permission denied. The same command passed outside that sandbox, proving an execution-environment failure rather than a source failure. This recurred once when the final portable fixture was accidentally run with workspace sandboxing: 23 tests passed and the same 2 socket tests failed. The immediate unsandboxed rerun passed all 25 tests.
3. Early `scripts/check.sh` runs found, in order: a false `ghostty` protocol-boundary match, a shifted lint-baseline line, stale P6/P7/P5 locked-protocol scope guards, Clippy `large_enum_variant`, and two Clippy `cloned_ref_to_slice_refs` findings. The boundary now blocks Ghostty crates without blocking the launch field name, the baseline was updated, prior-phase exceptions were narrowed to the P8 session module/export, the create payload was boxed, and the clones were removed.
4. Read-only review found seven material issues: incremental decoder allocation before per-kind header validation, replay truncation starting inside terminal control payloads, no policy recheck when an idle timer fires, malformed/non-terminal `sessionId` persistence loss, CPU counter regression reported as false zero, overbroad prior-phase protocol unlocks, and macOS-only verifier requirements in portable mode. All seven were fixed and covered by focused tests or verifier checks.
5. After the replay review fix, `session::replay::tests::replay_is_bounded_and_drops_partial_leading_line` initially returned `second\nthird` instead of `third`. Safe replay checkpoints now must be strictly after the discarded storage offset; the focused rerun passed.
6. A final combined build/bundle/staged shell command exited 1 without output, so its failing subcommand could not be identified. Running the exact component commands separately passed: app build, bundle verification, staging, staged lifecycle, and `git diff --check`.

**Final focused and phase-gate evidence:**

```text
cargo fmt --all && cargo test -p muxy-proto --locked --offline session && cargo test -p muxy-core --locked --offline session && cargo test -p muxy-core --locked --offline resources && cargo test -p muxy-core --locked --offline workspace_store && cargo test -p muxy-terminal --locked --offline offline
```

Passed after review fixes: protocol 25 passed, core session filter 5 passed, core resources 4 passed, core workspace store 11 passed, and terminal offline 6 passed.

```text
git diff --check
cargo fmt --all -- --check
scripts/verify-p8-terminal-memory.sh --fixture portable
scripts/verify-p8-terminal-memory.sh --self-test
```

Passed outside the restricted socket sandbox. The portable fixture reported protocol 25 passed, core session filter 5 passed, core resources 4 passed, terminal offline 6 passed, followed by `P8 portable fixture passed`. The self-test reported `P8 terminal memory verifier self-test passed`.

```text
scripts/check.sh
```

Passed after all review fixes. This included shell syntax, no-comments and boundary gates, P8 portable ownership, formatting, Clippy, workspace build/tests, prior verifier self-tests, the P8 self-test, debug app build, ad-hoc signing, and bundle verification.

```text
cargo check -p muxy-proto --all-features --locked --offline --target x86_64-unknown-linux-gnu
cargo check -p muxy-core --all-features --locked --offline --target x86_64-unknown-linux-gnu
cargo check -p muxy-terminal --all-features --locked --offline --target x86_64-unknown-linux-gnu
```

All three passed after the final review fixes.

```text
scripts/build-app.sh debug
scripts/verify-bundle.sh target/debug/Muxy.app debug
scripts/stage-test-app.sh target/debug/Muxy.app p8-phase-1
scripts/verify-p8-terminal-memory.sh --staged debug /Users/saeed/Projects/muxy-2.x/target/test-verification/apps/p8-phase-1/MuxyTests.app phase-1
git diff --check
```

All passed when run separately. The bundle passed strict code-sign verification. The staged app launched, created its injected P2 socket, answered retained CLI `list-projects`, created neither `sessions` nor `sessions-dev`, quit normally with status 0, removed its socket, and left no matching staged process. The verifier reported `P8 staged Phase 1 passed with zero staged process residue`. Final `git diff --check` passed.

**Unverified or intentionally inactive after Phase 1:**

- No daemon, PTY ownership, detach/attach, peer authentication, process-tree termination, or shell integration exists yet. These begin in Phase 2.
- No app startup transition, Ghostty attachment, idle runtime effect, resource monitor UI, session manager, or CLI session activation exists yet.
- Linux host runtime was not exercised. The three affected portable crates cross-target compiled only, as required for this phase.
- Native manual acceptance remains pending for the phases that introduce runtime behavior.

**Publication:** committed as `0c667be5` (`P8 phase 1: add portable session contracts`). The initial publication incorrectly pushed the commit directly to `origin/2.x` and opened draft PR #1105 from `2.x` against `main`. On user correction, remote `2.x` was restored to its pre-P8 commit `a122e440`, the work moved to `p8-terminal-memory-sessions`, PR #1105 was closed, and draft PR [#1106](https://github.com/muxy-app/muxy/pull/1106) titled `P8` was verified with base `2.x` and head `p8-terminal-memory-sessions`.

### Phase 2 - Secure daemon and detach/attach proof

**Completed:** 2026-08-31 08:46:00 UTC

**Implemented:**

- Added the `muxy-session` crate and helper binary with a same-user, versioned Unix transport; bounded framing; control and renderer clients; daemon lifecycle; secure runtime paths; Unix PTY ownership; replay; one active renderer; resize handling; shell integration; and macOS/Linux process-tree adapters.
- Added complete control operations, idempotent creation, build/protocol mismatch handling, independent control capacity, initial connection deadlines, daemon-owned attachment generations, renderer backpressure disconnection, natural-exit descendant cleanup, fixed-point TERM/KILL cleanup, PID reuse rejection, and bounded idle exit.
- Added isolated integration coverage for detach/reattach, replay and live output, deterministic renderer replacement, stale resize rejection, non-reading output caps, concurrent creation, lifecycle controls, runtime security, singleton preservation, connection limits and timeouts, shell transformations, changed process groups/sessions, natural shell exit, and exact cleanup.
- Added `docs/development/session-protocol.md` and bundled `Contents/MacOS/muxy-session` from the same profile. The helper is signed before the app. Bundle verification now checks the exact two-binary inventory, matching architectures, helper and app signatures, and all required shell integrations.
- Added Phase 2 P8 fixtures and a staged proof using an exclusive canonical `/tmp/p8-isolated-test-*` root with an ownership marker. Cleanup never searches or signals by process name. Rust records and revalidates kernel PID/start identities for daemon and shell cleanup. The staged app and Cargo probe are managed only as exact current shell jobs.
- Updated `scripts/check.sh`, lint baseline inventory, and prior P5/P6/P7 verifier ownership scopes only for the new crate and Phase 2-owned bundle/staging paths.

**Observed safety failure and corrective action:**

The first thread-hosted daemon tests could record a `forkpty` child before it became its own PTY session leader. The process tracker could therefore inherit and signal the parent terminal's OS session, affecting unrelated retained Swift Muxy sessions. The lifecycle tests were stopped. PTY creation now waits until child PID, parent PID, process group, process session, foreground PTY group, and nonzero TTY identity prove isolation. `ProcessTracker::new` independently rejects a non-isolated root. Integration daemons launch as separate `setsid()` child processes and use exclusive `/tmp/p8-isolated-test-*` roots. Failed PTY stabilization signals only the exact just-forked child PID and waits for it. No production or retained Swift runtime path, socket, daemon, or session is queried or cleaned. No process-name cleanup exists. The shortened isolated daemon tests, full process cleanup suite, and staged proof were rerun and passed.

**Observed failures and fixes:**

1. The first isolated test root exceeded the Unix socket path limit. A short exclusive `/tmp/p8-isolated-test-*` template replaced it.
2. A restricted filesystem/process sandbox rejected isolated socket daemon tests with `Operation not permitted`. The exact tests passed outside that sandbox. A later sandboxed `scripts/check.sh` run reproduced the two existing `muxy-proto` socket bind failures; the unsandboxed command passed.
3. The first concurrent end-all test hit the client's 3-second read timeout while cleaning two shells sequentially. The integration client's bounded timeout was raised to 10 seconds, and the focused test passed in about 6.39 seconds.
4. Clippy found `items_after_test_module` in `shell.rs` and `single_match` in `security.rs`. Both were fixed without lint suppression. After review, Clippy also rejected an intentionally orphaned test `Child` as a possible zombie; the isolated helper now uses direct `fork`/`setsid`/`exec`, and Clippy passed.
5. The first staged run failed with `error: invalid owned PID` because the script's global newline-only `IFS` prevented space-separated record parsing. Record reads now set `IFS=' '` locally.
6. The next staged run failed because the test killed the daemon directly, leaving `control.sock`. It now ends the session and waits for the daemon's normal 10-second idle exit, allowing `SecureRuntime` to remove its exact live socket.
7. `scripts/check.sh` successively found duplicated `sessions-dev` and `debug_assertions` policy ownership, a shifted lint baseline, stale P6 crate/binary inventory, and P6/P7/P5 locked bundle path scopes. Runtime tests now consume `RuntimePathPolicy`, build mode uses `muxy_core::build_mode!()`, inventories and line numbers are current, and only Phase 2-owned paths were removed from prior locked lists. Focused P5, P6, and P7 self-tests and the final full check passed.
8. A grouped post-review exact-gate command exited 1 without output, so the failing component was not observable. Every exact gate component was rerun separately and passed. This mirrors an earlier grouped build/bundle/staged command whose separately run components also passed.

**Read-only review and fixes:**

The required one-time read-only Phase 2 review reported seven material findings. All were verified and addressed without a second review cycle:

1. Runtime-bearing tests now consistently use exclusive prefixed temporary roots, and the staged runtime moved from a reusable repository path to an exclusive canonical temporary root with ownership validation.
2. Staged cleanup no longer hashes second-resolution `ps lstart` output. Rust records kernel process start identities and revalidates the exact identity immediately before TERM and KILL. Direct staged jobs are validated through the shell job table rather than process names.
3. Secure runtime operations now canonicalize the trusted parent once, open the owned final leaf descriptor-relatively with `O_NOFOLLOW`, retain its device/inode, use the canonical validated socket path, and revalidate directory and socket identity around stale recovery, bind, cleanup, and drop.
4. Process-tree termination now propagates every snapshot failure, uses successful snapshots for signaling and final absence proof, and cannot acknowledge cleanup after an unverified rescan.
5. Natural shell exit now runs the same fixed-point descendant cleanup before marking the descriptor exited or allowing daemon idle exit. A real detached descendant test passes.
6. Attachment identity is daemon-assigned and unique. A reused client proposal cannot let an old renderer detach, input to, or resize its replacement.
7. Accepted connections have a bounded deadline through authentication, `Hello`, and the required first operation. A 128-partial-handshake test proves capacity recovers.

**Final focused and phase-gate evidence:**

```text
cargo test -p muxy-session --all-targets --all-features --locked --offline
```

Passed after review fixes: 11 unit tests, 2 attach tests, 2 daemon tests, 4 process-cleanup tests, 5 security tests, and the staged-helper no-op test. The singleton test intentionally prints `Resource temporarily unavailable (os error 35)` from the rejected second daemon while proving the first socket remains usable.

```text
scripts/verify-p8-terminal-memory.sh --fixture protocol
scripts/verify-p8-terminal-memory.sh --fixture security
scripts/verify-p8-terminal-memory.sh --fixture daemon-detach-attach
scripts/verify-p8-terminal-memory.sh --fixture process-cleanup
scripts/verify-p8-terminal-memory.sh --fixture shell-integration
```

All passed. The verifier reported protocol, security with isolated roots, daemon detach/attach with isolated roots, identity-bound process cleanup, and shell integration success.

```text
scripts/check.sh
```

Passed after the prior-verifier scope fixes. This included P8 portable and every Phase 2 fixture, formatting, Clippy with warnings denied, workspace build/tests, staging safety, prior verifier self-tests, debug app build, helper-first ad-hoc signing, and bundle verification.

```text
cargo check -p muxy-session --all-features --locked --offline --target x86_64-unknown-linux-gnu
scripts/build-app.sh debug
scripts/verify-bundle.sh target/debug/Muxy.app debug
staged_app="$(scripts/stage-test-app.sh target/debug/Muxy.app p8-phase-2)"
scripts/verify-p8-terminal-memory.sh --staged debug "$staged_app" phase-2
git diff --check
cargo fmt --all -- --check
```

All passed separately after review fixes. The staged app path was `/Users/saeed/Projects/muxy-2.x/target/test-verification/apps/p8-phase-2/MuxyTests.app`. The staged proof launched the app in an injected root, answered retained CLI `list-projects`, started the bundled helper in a separate OS session, created and detached a daemon-owned shell, proved exact daemon and shell identities remained alive while the app closed normally, reattached for replay and new live output, ended the session, waited for normal daemon idle exit, removed both app and session sockets, and left no `/tmp/p8-isolated-test-*` residue.

**Unverified or intentionally deferred after Phase 2:**

- Linux host runtime was not launched. The complete crate cross-target compiled for `x86_64-unknown-linux-gnu`, as required.
- App startup transitions, workspace linkage, Ghostty attachment, visible-only restoration, settings effects, session lifecycle UX, idle sleeping runtime, resource monitoring, session manager, and CLI session activation begin in later phases.
- Attach proxy stdin and resize threads are process-lifetime threads and are not joined; the `sigwait` thread remains blocked until process exit.
- The PTY child still resets signals, changes directory, closes descriptors, and calls `execve` after `forkpty`; no alternate spawn architecture was introduced.
- Replay chunks use a synthetic contiguous sequence range ending at the replay snapshot's last sequence rather than preserving original PTY read chunk boundaries.
- Linux-host peer credential, PTY, and process enumeration behavior and notarized distribution signing remain outside this phase's observed runtime evidence.

**Publication:** committed as `9099f676` (`P8 phase 2: add secure background sessions`) and pushed to draft PR [#1106](https://github.com/muxy-app/muxy/pull/1106), base `2.x`, head `p8-terminal-memory-sessions`.

### Phase 3 - Restart-only mode transition and Ghostty attachment

**Completed:** 2026-08-31 11:38:03 UTC

**Implemented:**

- Added one `SessionCoordinator` before window construction with an explicit startup barrier, desired versus applied restart-only mode, profile-specific isolated runtime selection, daemon startup and recovery, exact-owner workspace reconciliation, durable session links, and explicit `Present`, `Missing`, `Ended`, and `AttachmentFailed` states.
- Eligible local workspace terminal tabs become daemon-owned before renderer reconciliation. Remote projects and Quick Terminal remain excluded. Hidden persistent tabs remain daemon-backed and surface-free, while only visible tabs materialize Ghostty attachment proxies through the bundled `muxy-session attach` helper.
- Added proxy-only exit handling, exact linked-session refresh, in-process attachment retry without creating a replacement session, unavailable state copy, and Retry UI. A daemon-ended session closes its tab, while missing, running, and query-error cases preserve the tab.
- Added restart-required Settings behavior. Enabling only changes desired mode for the next launch. Disabling counts active daemon sessions, requires destructive confirmation when the count is nonzero, ends daemon descendants before clearing durable IDs, then starts ordinary terminal surfaces.
- Added bounded client and coordinator connection handshakes, test-daemon readiness polling through connect plus ping, preservation of the requested normal `MUXY_SOCKET_PATH` through PTY environment scrubbing, and a macOS no-device sentinel conversion that prevents unrelated no-TTY processes from sharing a false TTY identity.
- Added Phase 3 source guards, `transitions`, `renderer-attachment`, and `exclusions` fixtures, and a staged restart lifecycle proof. Every staged runtime uses a unique `/tmp/p8-isolated-test-*` root. Cleanup signals only exact test-owned shell jobs or recorded PID/start identities and never searches by process name.

**Observed failures and fixes:**

1. An initial isolated daemon connection saw `ConnectionRefused` after the socket file appeared but before the listener was ready. `TestDaemon::start` now retries `SessionClient::connect` and `ping` against only its unique socket until the existing three-second deadline. The complete six-test security suite passed three consecutive runs after this fix.
2. A focused stalled-server test and one `scripts/check.sh` attempt failed in restricted sandboxes with `Operation not permitted` while binding Unix sockets. Unsandboxed reruns passed. The final sandboxed `scripts/check.sh` attempt similarly failed two existing `muxy-proto` socket tests under `target/test-verification`; the immediate unsandboxed exact rerun passed the full check.
3. Direct conversion of macOS `proc_bsdinfo.e_tdev == u32::MAX` could make unrelated no-TTY processes appear to share a TTY. The sentinel now maps to zero and a regression test proves it is not a shared TTY identity.
4. Initial focused compilation found `"Disable Background Sessions?"` had type `&str` where `String` was required. The title is now owned. A focused confirmation test then found singular copy used `inside them`; singular copy now uses `inside it`.
5. Bounded client handshakes changed control-capacity rejection on macOS from a post-handshake failed ping to an `InvalidInput` connect error. The security test now accepts either bounded rejection point and still proves capacity recovery after one permit is released.
6. An intermediate startup-race fix used a mutable client in a pattern guard and failed compilation. The ping moved into the normal match body.
7. An earlier staged Phase 3 attempt exceeded the Unix socket path limit. The staged verifier now uses a short unique `/tmp/p8-isolated-test-staged-phase3.*` root.
8. The exact `muxy` Linux cross-check failed before project code because this macOS host lacks `x86_64-linux-gnu-gcc` and `x86_64-linux-gnu-g++`, affecting `psm` and `freetype-sys`. This failure remains part of the gate result. A supplemental isolated Zig-wrapper cross-check passed with 21 pre-existing Linux warnings.
9. A final GitHub PR metadata query before this phase failed with `error connecting to api.github.com`. Publication metadata must be rechecked after push when connectivity is available.

**Read-only review and fixes:**

The required one-time read-only Phase 3 review reported five material findings. All were verified and addressed without a second review cycle:

1. Destructive disable now counts running background sessions and confirms the exact scope before persisting false.
2. Client handshakes and coordinator startup control I/O are bounded, so a stalled socket cannot hold app startup indefinitely.
3. Attachment failures expose an in-process Retry action that reuses only the exact running linked daemon session.
4. Staged verifier error paths stop and wait for only their exact shell-owned app job, preventing leaked test apps without process-name cleanup.
5. Staged evidence now observes visible renderer output, hidden surface absence, tab switching, exact session IDs across reopen, replay, restart-only disable behavior, and final exact-root cleanup rather than inferring those behaviors.

**Final focused and phase-gate evidence:**

```text
cargo test -p muxy-core --locked --offline session
cargo test -p muxy-terminal --locked --offline offline
cargo test -p muxy --locked --offline sessions
cargo test -p muxy --locked --offline terminal
scripts/verify-p8-terminal-memory.sh --fixture transitions
scripts/verify-p8-terminal-memory.sh --fixture renderer-attachment
scripts/verify-p8-terminal-memory.sh --fixture exclusions
```

All passed after the final source revisions: core session 5 passed, terminal offline 6 passed, Muxy sessions 7 passed, Muxy terminal 77 passed, and all three fixtures reported success.

```text
scripts/check.sh
```

The first final attempt under workspace sandboxing failed because two existing `muxy-proto` server tests could not bind their temporary Unix sockets and returned `Operation not permitted`. The exact unsandboxed rerun passed. It included all P8 fixtures, formatting, Clippy, workspace tests, verifier self-tests, debug app build, helper-first signing, and bundle verification. The security fixture passed all 6 tests, including the bounded stalled-server handshake and daemon startup race coverage.

```text
cargo check -p muxy-session --all-features --locked --offline --target x86_64-unknown-linux-gnu
```

Passed.

```text
cargo check -p muxy --all-features --locked --offline --target x86_64-unknown-linux-gnu
```

Failed before checking project code because `x86_64-linux-gnu-gcc` and `x86_64-linux-gnu-g++` are not installed on this host. `psm` and `freetype-sys` reported the missing native cross compilers.

```text
PATH="$PWD/target/test-verification/p8/toolchain:$PATH" AR_x86_64_unknown_linux_gnu='zig ar' cargo check -p muxy --all-features --locked --offline --target x86_64-unknown-linux-gnu
```

The supplemental Zig-wrapper cross-check passed with 21 existing Linux-only unused or dead-code warnings.

```text
scripts/build-app.sh debug
scripts/verify-bundle.sh target/debug/Muxy.app debug
staged_app="$(scripts/stage-test-app.sh target/debug/Muxy.app p8-phase-3)"
scripts/verify-p8-terminal-memory.sh --staged debug "$staged_app" phase-3
git diff --check
find /tmp -maxdepth 1 -name 'p8-isolated-test-*' -print
```

All executable checks passed. The staged app path was `/Users/saeed/Projects/muxy-2.x/target/test-verification/apps/p8-phase-3/MuxyTests.app`. The verifier reported `P8 staged Phase 3 restart-only transition, hidden/visible renderer, attachment replay, exact-ID reopen, and disable cleanup passed`. `git diff --check` passed and the final isolated-root search returned no output.

The staged proof used retained CLI commands to observe at least two durable IDs, a working visible renderer, hidden-tab `pane surface not ready`, tab activation and live output, exact IDs and replay after reopen, no live disable transition before restart, then durable-link and daemon-socket cleanup after restart-disabled startup.

**Unverified or intentionally deferred after Phase 3:**

- Native Linux-host daemon, peer credential, PTY, renderer, and app runtime behavior was not exercised. `muxy-session` cross-target compiled with the host toolchain, and the full app cross-target compiled only through the supplemental Zig wrappers because the exact native cross compilers are absent.
- Native manual UI acceptance for startup-blocked controls, Settings confirmation presentation, attachment failure copy, and Retry interaction remains pending.
- Idle sleeping, adaptive replay, resource monitoring, Session Manager, CLI activation, final workflow coverage, and release acceptance remain assigned to later phases.

**Publication target:** Phase 3 commit `P8 phase 3: integrate persistent terminal sessions` on `p8-terminal-memory-sessions`, pushed to draft PR [#1106](https://github.com/muxy-app/muxy/pull/1106) with required base `2.x`.

### Phase 4 - Full workspace and background-session lifecycle

**Completed:** 2026-08-31 13:14:02 UTC

**Implemented:**

- Added immutable close and exact-owner cleanup plans. Single-tab and every `CloseMode` compute the complete candidate set before daemon cleanup, revalidate it before ending any session, mutate only after acknowledgements, and report persistence failures without claiming rollback.
- Added Send to Background, exact Focus/Reattach, Start New Terminal, managed session state models, exact original owner placement restoration, and distinct Workspace, Background, Missing, Ended, and AttachmentFailed outcomes. Reattach and Start New do not replay an old startup command.
- Added project and worktree deletion transactions that hold and revalidate operation tokens around counted immutable cleanup plans. Candidate additions or owner changes after confirmation abort before any session is ended. Project mutation occurs while the token is active.
- Added explicit project-removal workspace persistence outcomes and a Retry Save action. Sessions stay ended after a partial persistence failure and the UI does not report rollback.
- Added neutral current-generation project/worktree existence facts. Two increasing Missing observations in one generation are required before exact-owner cleanup. Present, Unknown, stale observations, symlink loops, and permission errors fail safe.
- Added app startup and activation truth probes, normal quit detach-only behavior, exact End All separation, missing/ended placeholders, and recoverable Start New and Remove actions.
- Added isolated lifecycle integration coverage and six Phase 4 fixtures. The staged proof now uses only the staged app's isolated session socket and app lifecycle hook. It sends an app-owned session to Background, closes normally, reopens and reattaches the exact ID and original placement, performs token-held project deletion cleanup while preserving an unrelated project session, runs End All, and verifies exact recorded PID/start identities are dead before removing their record.
- Every runtime proof used a unique `/tmp/p8-isolated-test-*` root. No production or Swift runtime path was queried or cleaned. Cleanup never searched by process name and signaled only recorded exact test-owned identities.

**Observed failures and fixes:**

1. An early focused `cargo test -p muxy-session --locked --offline lifecycle` run in the workspace sandbox failed before daemon acceptance with `Operation not permitted`. The unsandboxed rerun exposed a real timeout because `startup_command = "exit 7"` did not produce the intended shell status. The test now uses `/bin/sh -c 'exit 7'`; the unsandboxed lifecycle test and final exact gate passed.
2. An intermediate workspace-view edit had an unclosed delimiter. Formatting and compilation identified it, the delimiter was fixed, and subsequent formatting, checks, Clippy, and tests passed.
3. The exact full-app Linux cross-check failed before project code because this macOS host lacks `x86_64-linux-gnu-gcc` and `x86_64-linux-gnu-g++`. `psm` and `freetype-sys` reported the missing tools. The required failure remains part of the gate result. The supplemental Zig-wrapper check passed with 21 existing Linux-only warnings.

**Read-only review and fixes:**

The required one-time read-only Phase 4 review reported four high-confidence findings. All were verified and fixed without a second review cycle:

1. The staged proof previously drove a separate helper daemon directly. It now seeds two isolated app projects and drives Background, reopen reattach, project deletion cleanup, and End All through the staged app coordinator and app-owned session socket.
2. The prior helper could fail before recording detached process identities. The direct Phase 4 helper daemon was removed. The app hook records its daemon and every running shell identity before lifecycle mutation, error cleanup uses the isolated app disable path plus exact identities, and final verification removes the record only after every exact identity is dead.
3. Project/worktree confirmation counts previously came from a different daemon snapshot than cleanup. Both flows now retain immutable exact session ID plus full owner plans, relist and compare the complete set after confirmation, and abort before ending anything if it changed.
4. Project deletion persistence failure was previously logged and treated as success. It now returns a structured partial-success outcome and presents an explicit Retry Save action without rerunning cleanup or resurrecting sessions.

**Final focused and phase-gate evidence:**

```text
cargo test -p muxy-api --locked --offline truth
cargo test -p muxy-core --locked --offline reconciliation
cargo test -p muxy-session --locked --offline lifecycle
cargo test -p muxy --locked --offline session_lifecycle
scripts/verify-p8-terminal-memory.sh --fixture close-background-reattach
scripts/verify-p8-terminal-memory.sh --fixture all-close-modes
scripts/verify-p8-terminal-memory.sh --fixture owner-deletion-transaction
scripts/verify-p8-terminal-memory.sh --fixture external-owner-truth
scripts/verify-p8-terminal-memory.sh --fixture quit-crash-missing
scripts/verify-p8-terminal-memory.sh --fixture startup-command-once
```

All passed in exact order. Truth reported 3 passed, reconciliation 3 passed, daemon lifecycle 1 passed, Muxy session lifecycle 7 passed, and all six fixtures reported success.

```text
scripts/check.sh
```

Passed unsandboxed. It completed shell/source guards, all workspace tests, P8 runtime fixtures, formatting, Clippy, verifier self-tests, debug app build, helper-first signing, and bundle verification.

```text
cargo check -p muxy-api --all-features --locked --offline --target x86_64-unknown-linux-gnu
cargo check -p muxy-session --all-features --locked --offline --target x86_64-unknown-linux-gnu
```

Both passed.

```text
cargo check -p muxy --all-features --locked --offline --target x86_64-unknown-linux-gnu
```

Failed before project code because `x86_64-linux-gnu-gcc` and `x86_64-linux-gnu-g++` are absent. The failures came from `psm` and `freetype-sys` native build scripts.

```text
PATH="$PWD/target/test-verification/p8/toolchain:$PATH" AR_x86_64_unknown_linux_gnu='zig ar' cargo check -p muxy --all-features --locked --offline --target x86_64-unknown-linux-gnu
```

The supplemental cross-check passed with 21 existing Linux-only unused or dead-code warnings.

```text
scripts/build-app.sh debug
scripts/verify-bundle.sh target/debug/Muxy.app debug
staged_app="$(scripts/stage-test-app.sh target/debug/Muxy.app p8-phase-4)"
scripts/verify-p8-terminal-memory.sh --staged debug "$staged_app" phase-4
git diff --check
find /tmp -maxdepth 1 -name 'p8-isolated-test-*' -print
```

All executable checks passed. The staged app path was `/Users/saeed/Projects/muxy-2.x/target/test-verification/apps/p8-phase-4/MuxyTests.app`. The verifier reported `P8 staged Phase 4 app-owned Background survival, exact reattach, project deletion cleanup, End All, and zero residue passed`. `git diff --check` passed and the final isolated-root search returned no output.

Supplemental final checks also passed:

```text
cargo clippy -p muxy-api -p muxy-session -p muxy --all-targets --all-features --locked --offline -- -D warnings
bash -n scripts/verify-p8-terminal-memory.sh
```

**Unverified or intentionally deferred after Phase 4:**

- Native Linux-host daemon, PTY, renderer, app runtime, and process identity behavior were not executed. API and session crates cross-target compiled with the host toolchain. The full app required supplemental Zig wrappers because the exact native cross compilers are absent.
- Manual UI interaction for destructive close confirmations, Send to Background menu presentation, persistence retry prompts, attachment-failure copy, and project/worktree cancellation remains unobserved. Their state and transaction paths are covered by focused tests and fixtures.
- Idle renderer sleeping, resource monitoring, status UI, Session Manager, CLI session activation, profile hardening, and release acceptance remain assigned to Phases 5 through 8.

**Publication target:** Phase 4 commit `P8 phase 4: complete session lifecycle` on `p8-terminal-memory-sessions`, pushed to draft PR [#1106](https://github.com/muxy-app/muxy/pull/1106), base `2.x`.

### Phase 5 - Idle Ghostty surface sleeping and wake

**Completed:** 2026-08-31 15:41:49 UTC

**Implemented:**

- Added a hidden-only idle coordinator with generation-bound timers, complete eligibility rechecks, bounded FIFO wake operations, adjacent resize coalescing, and production resize-before-input replay.
- Counted input, output, focus, visibility, resize, action, and materialization as activity. Runtime proof showed Ghostty redraw state alone can miss output deliveries, so the narrow data callback increments only activity generation.
- Added immutable process identities and macOS process sampling using current-user enumeration plus exact foreground ancestry. Foreground children, related background descendants, alternate screen, incomplete samples, duplicate PIDs, PID reuse, and ambiguous surviving roots fail awake.
- Added support for protected or short-lived terminal session launchers. A shell below an unavailable launcher can be identified from exact session and TTY facts, while an ended launcher is replaced only by one unambiguous surviving shell root.
- Persistent sleep drops only the renderer and attach proxy while retaining the session ID and daemon shell identity. Wake reattaches with replay.
- Ordinary sleep is allowed only for a safe idle shell, records the latest cwd, suppresses the original startup command, and documents scrollback loss. Visible unfocused panes remain live.
- Added typed settings, one-second polling, workspace visibility and focus synchronization, and isolated Phase 5 fixtures plus staged app verification.
- All staged runtime paths used unique `/tmp/p8-isolated-test-*` roots. Cleanup used only exact test-owned roots and recorded PID/start identities. No production or Swift Muxy runtime path was queried, signaled, or cleaned.

**Observed failures and fixes:**

1. Early staged runs reported `hidden persistent renderer remained after the idle timeout`. Process sampling initially queried inaccessible unrelated processes, then treated exited short-lived attach proxies as unknown. Sampling now uses current-user enumeration, exact ancestry expansion, immutable start identities, and a safe ended-proxy result only when the stale foreground PID is absent. Present reused PIDs remain unknown.
2. A sandboxed staged run failed to bind `/private/tmp/p8-isolated-test-*/app/muxy-dev.sock` with `Operation not permitted`. The preserved root was inspected and removed only after its ownership marker matched. Unsandboxed staged runs reached the feature proof.
3. Ordinary staged runs initially remained awake because protected session launchers were absent from current-user snapshots. Exact parent, session, TTY, and user facts now identify the shell below that boundary and retain ambiguity as unknown.
4. The staged ordinary sleep check produced a false failure after sleeping because `read-screen` is an action that wakes and synchronously rematerializes an ordinary shell. The isolated verifier now observes the exact test-only sleep event before issuing the wake action, then still proves cwd restoration.
5. The first final Linux terminal check emitted one unused-function warning for a macOS-only replacement helper. The helper is now target-gated and the exact check reran cleanly.
6. The exact full-app Linux cross-check failed before project code because this host lacks `x86_64-linux-gnu-gcc` and `x86_64-linux-gnu-g++`. `freetype-sys` and `psm` reported the missing tools. A sandboxed Zig fallback also failed because Zig could not write `~/.cache/zig`; the unsandboxed fallback passed with 21 existing Linux-only warnings.
7. A final shell wrapper reported exit 1 after `scripts/check.sh` completed successfully because zsh reserves the variable name `status`. The check itself passed. The earlier exact invocation also passed.

**Read-only review and fixes:**

The required one-time Phase 5 review reported Phase 5 as partially achieved with three findings. All were fixed without a second review cycle:

1. Ordinary process sampling could miss reparented or background descendants, incomplete samples, and PID reuse. Exact immutable roots, session/group/TTY association, recursive descendants, truncation failure, ambiguity rejection, and replacement-root rules now fail safe.
2. Final-grid fingerprint polling could miss output activity. Runtime proof reproduced that gap, so the Ghostty data callback now increments an atomic activity generation and is unregistered before its backing allocation is dropped.
3. Resize wake ordering existed only in tests. Production now records grid size, queues resize before wake input, applies operations in order during materialization, coalesces only adjacent resizes, and restores failed and remaining operations.

**Final phase-gate evidence:**

```text
cargo test -p muxy-terminal --all-targets --all-features --locked --offline offline
cargo test -p muxy --locked --offline terminal_idle
scripts/verify-p8-terminal-memory.sh --fixture idle-policy
scripts/verify-p8-terminal-memory.sh --fixture idle-races
scripts/verify-p8-terminal-memory.sh --fixture idle-process-safety
scripts/verify-p8-terminal-memory.sh --fixture persistent-sleep-wake
scripts/verify-p8-terminal-memory.sh --fixture ordinary-sleep-wake
scripts/check.sh
```

The terminal offline suite passed 17 tests, the Muxy idle suite passed 11 tests, all five fixtures reported success, and `scripts/check.sh` completed all source guards, workspace tests, verifier self-tests, debug bundle build, signing, and bundle verification.

```text
cargo check -p muxy-terminal --all-features --locked --offline --target x86_64-unknown-linux-gnu
```

Passed cleanly.

```text
cargo check -p muxy --all-features --locked --offline --target x86_64-unknown-linux-gnu
```

Failed before project code because the required GNU cross C and C++ compilers are absent.

```text
PATH="$PWD/target/test-verification/p8/toolchain:$PATH" AR_x86_64_unknown_linux_gnu='zig ar' cargo check -p muxy --all-features --locked --offline --target x86_64-unknown-linux-gnu
```

The supplemental unsandboxed cross-check passed with 21 existing Linux-only warnings.

```text
scripts/build-app.sh debug
scripts/verify-bundle.sh target/debug/Muxy.app debug
staged_app="$(scripts/stage-test-app.sh target/debug/Muxy.app p8-phase-5)"
scripts/verify-p8-terminal-memory.sh --staged debug "$staged_app" phase-5
git diff --check
find /private/tmp -maxdepth 1 -name 'p8-isolated-test-*' -print
```

All executable checks passed. The staged app path was `/Users/saeed/Projects/muxy-2.x/target/test-verification/apps/p8-phase-5/MuxyTests.app`. The verifier reported `P8 staged Phase 5 persistent sleep/replay identity, ordinary process safety, cwd wake, and zero residue passed`. `git diff --check` passed and the final isolated-root search returned no output.

Supplemental strict checks also passed:

```text
cargo clippy -p muxy-terminal -p muxy --all-targets --all-features --locked --offline -- -D warnings
bash -n scripts/verify-p8-terminal-memory.sh
```

**Unverified or intentionally deferred after Phase 5:**

- Native Linux-host renderer, process-tree sampling, sleeping, and wake behavior were not executed. The terminal crate cross-target compiled natively, while the full app required the supplemental Zig wrappers.
- Manual UI interaction for the idle settings disclosure, visible-unfocused panes, ordinary scrollback loss, and wake latency remains unobserved. Focused tests, fixtures, and the staged app cover the underlying state and runtime paths.
- Resource monitoring, status UI, Session Manager, CLI activation, profile hardening, and release acceptance remain assigned to Phases 6 through 8.

**Publication target:** Phase 5 commit `P8 phase 5: sleep idle terminal surfaces` on `p8-terminal-memory-sessions`, pushed to draft PR [#1106](https://github.com/muxy-app/muxy/pull/1106), base `2.x`.

### Phase 6 - Process-tree resource monitor and status item

**Completed:** 2026-08-31 17:01:46 UTC

**Implemented:**

- Added portable exact-identity process-tree aggregation in `muxy-core`. Parent edges carry full PID/start identities, conflicting duplicate records fail closed, overlapping roots deduplicate, resident-byte sums saturate, and CPU deltas preserve unavailable state for missing baselines, PID reuse, regressions, and zero elapsed time.
- Added a macOS one-second sampler using current-user `proc_listpids`, `proc_pidinfo`, and `proc_pid_rusage`. It validates child and parent identities around each sample, tolerates only exact process disappearance, and reports cumulative user plus system CPU time and resident bytes. Unsupported targets return an explicit unavailable result.
- Added `ResourceMonitor` with generation-bound requests, fresh baselines after enable transitions, live, stale, and unavailable snapshots, app/session/total aggregates, transient app-identity retry, and immediate clearing plus poll cancellation when disabled.
- Added authenticated daemon and running-shell roots from `SessionCoordinator`. Daemon IPC and process sampling run on the background executor rather than the GPUI thread.
- Added the typed `muxy.showResourceUsageInStatusBar` runtime effect, neutral compact CPU/RAM status text, explicit stale/unavailable copy, detailed tooltip totals, and ordered coexistence with the Composer footer in both standalone and merged status bars.
- Added portable math and process-tree fixtures plus a staged app proof. The staged verifier uses a unique owned `/tmp/p8-isolated-test-*` root, records exact daemon, shell, and sampled descendant PID/start identities atomically, creates a real session-owned CPU load, proves nonzero CPU/RAM and increased process count, disables sampling through the production setting effect, proves polling stops, and verifies every recorded identity is dead before removing the owned root.
- Updated the prior Composer source guard only because the Phase 6 status group changed the shared trailing status API from one optional element to an ordered vector.

**Observed failures and fixes:**

1. The first `cargo check -p muxy --offline` updated `Cargo.lock` for the direct macOS `libc` dependency and then found an untyped `HashMap`; the map type was made explicit and the check passed.
2. An initial macOS sampler test exposed short-lived process disappearance where the API result did not retain `ESRCH`. The sampler now rechecks only the exact PID with signal 0 and treats it as absent only when that exact identity has disappeared. Focused macOS tests then passed.
3. The first `scripts/check.sh` found ShellCheck `SC2015` for an `A && B || C` expression. It was replaced with explicit conditional logic. The next run found the shifted `Cargo.toml` lint-baseline line, and the baseline was updated from 41 to 42. The following run found the prior Composer verifier's locked single trailing element; its integration guard was updated to the new vector API. The resulting full check passed.
4. During the sampled-identity review fix, `cargo test -p muxy --locked --offline resource_monitor` failed because one test initializer lacked the new `session_roots` and `session_processes` fields. Both fields were added and the complete focused rerun passed.
5. Two staged review-fix attempts tried to obtain the load PID through terminal output and then through `/tmp/p8-isolated-test-*/phase6-load-pid`. They failed with `staged Phase 6 descendant load did not report its PID` and `staged Phase 6 descendant load did not report its PID and terminal token`; the latter also printed a nonexistent-file redirection error. The test-only PID injection path was removed. Production process-tree samples now atomically publish every exact session-tree identity, including the descendant, and the staged proof passed.
6. The final sandboxed `scripts/check.sh` attempt reached the P8 portable fixture but two existing `muxy-proto` server tests could not bind temporary Unix sockets under `target/test-verification`, returning `SocketBind` with `Operation not permitted`. The exact unsandboxed rerun passed the complete check.
7. The exact full-app Linux cross-check failed before project code in `freetype-sys v0.20.1` and `psm v0.1.32` because `x86_64-linux-gnu-g++` and `x86_64-linux-gnu-gcc` are absent. Exit status was 101. The documented unsandboxed Zig-wrapper supplemental check passed with 21 existing Linux-only warnings.
8. GitHub metadata lookup before implementation failed with `error connecting to api.github.com`; PR metadata remains to be verified after publication when connectivity is available.
9. The first commit staging command named tracked `.plans/P8-terminal-memory-sessions.md` directly, but `.plans` is ignored and Git rejected that path without staging anything. The tracked plan update was staged with `git add -u` instead.

**Read-only review and fixes:**

The required one-time read-only Phase 6 review reported four material findings. All were verified and fixed without a second review cycle:

1. Process-tree edges previously authenticated only the parent PID. Every edge now carries and revalidates the parent's exact PID/start identity, and reuse coverage proves a child cannot attach to a reused parent.
2. The staged proof previously inferred descendant sampling without preserving its identity and could race before shell roots were recorded. Successful production samples now expose exact root and tree identities, atomically rewrite the owned identity record, and require one daemon, at least one shell, at least one descendant, and a record count equal to the sampled session process count before cleanup.
3. Session-root daemon IPC previously ran synchronously on the GPUI thread. A cloned coordinator now performs it on the background executor before the UI-bound request is created.
4. A transient initial app-identity failure previously ended monitoring permanently. Enabled monitors now retry identity acquisition once per interval and the monitor loop continues while no request is available.

**Final focused and phase-gate evidence:**

```text
cargo fmt --all
bash -n scripts/verify-p8-terminal-memory.sh
cargo test -p muxy-core --locked --offline resources
cargo test -p muxy --locked --offline resource_monitor
cargo test -p muxy-session --test staged_helper --locked --offline
scripts/verify-p8-terminal-memory.sh --fixture resource-math
scripts/verify-p8-terminal-memory.sh --fixture resource-process-tree
```

Passed after the final review fix: core resources 6 passed, Muxy resource monitor 7 passed, staged helper 3 passed, and both fixtures reported success.

The exact Phase 6 gate was then run in the plan's order:

```text
cargo test -p muxy-core --locked --offline resources
cargo test -p muxy --locked --offline resource_monitor
scripts/verify-p8-terminal-memory.sh --fixture resource-math
scripts/verify-p8-terminal-memory.sh --fixture resource-process-tree
scripts/check.sh
cargo check -p muxy-core --all-features --locked --offline --target x86_64-unknown-linux-gnu
cargo check -p muxy --all-features --locked --offline --target x86_64-unknown-linux-gnu
scripts/build-app.sh debug
scripts/verify-bundle.sh target/debug/Muxy.app debug
staged_app="$(scripts/stage-test-app.sh target/debug/Muxy.app p8-phase-6)"
scripts/verify-p8-terminal-memory.sh --staged debug "$staged_app" phase-6
git diff --check
```

The first four commands passed with 6 core tests, 7 monitor tests, and both fixture success messages. The unsandboxed `scripts/check.sh` rerun passed shell and source guards, formatting, Clippy with warnings denied, workspace build/tests, all verifier self-tests, debug app build, helper-first signing, and bundle verification. The `muxy-core` Linux cross-check passed. The exact full-app Linux check produced the documented missing-GNU-compiler environmental failure.

The supplemental command was run unsandboxed:

```text
PATH="$PWD/target/test-verification/p8/toolchain:$PATH" \
AR_x86_64_unknown_linux_gnu='zig ar' \
cargo check -p muxy --all-features --locked --offline \
--target x86_64-unknown-linux-gnu
```

It passed with 21 existing Linux-only unused or dead-code warnings.

The debug app build and strict bundle verification passed. Staging produced `/Users/saeed/Projects/muxy-2.x/target/test-verification/apps/p8-phase-6/MuxyTests.app`, and the verifier reported `P8 staged Phase 6 app, daemon, shell, descendant resource sampling, disable stop, and zero residue passed`. Final `git diff --check` passed.

**Unverified or intentionally deferred after Phase 6:**

- Native Linux-host process enumeration, daemon tree sampling, and app runtime behavior were not executed. Portable core code cross-target compiled directly, while the full app required the supplemental Zig wrappers.
- Manual visual acceptance of status-bar density, tooltip presentation, live updates, stale/unavailable copy, and Composer coexistence remains unobserved. Focused tests and the staged runtime cover state, ordering, sampling, disable behavior, and cleanup.
- A forced production sampling failure was covered by monitor tests but not induced in the staged app. The staged proof observed live sampling and disabled/unavailable clearing.
- Session Manager, CLI activation, final profile hardening, release documentation, and release acceptance remain assigned to Phases 7 and 8.

**Publication target:** Phase 6 commit `P8 phase 6: add process resource monitoring` on `p8-terminal-memory-sessions`, pushed to draft PR [#1106](https://github.com/muxy-app/muxy/pull/1106), base `2.x`.

### Phase 7 - Session-manager popover and P2 CLI activation

**Completed:** 2026-08-31 18:33:13 UTC

**Implemented:**

- Added a status-bar Session Manager with deterministic Workspace Sessions, Background Sessions, and Missing/Ended sections. Focus, Reattach, End, Start New, Remove, End All, and Terminal Settings use production coordinator operations with exact action IDs, counted confirmations, and accessibility labels.
- Defined attached as durable workspace placement. Hidden renderer-free terminal tabs remain attached. The status control follows the applied launch mode, remains available until a restart actually disables sessions, and supports mouse, Enter, and Space activation.
- Added exact-owner session ending that validates the selected daemon descriptor, prevalidates durable tab removal, ends only the exact daemon tree, persists the updated workspace store, and removes stale owner-changed links safely. Bulk ending validates the complete confirmed set before signaling and persists all corresponding tab removals.
- Activated the retained `list-sessions` and `kill-session` P2 CLI heads without changing the wrapper bytes, framing, terminator convention, command-head count, or legacy bundle path. Listing emits the pinned tab-separated session, shell, working-directory, attachment, title, project, worktree, and tab columns. Killing reports exact usage, invalid UUID, not-found, owner mismatch, unavailable-runtime, and success results, and removes an attached workspace tab after exact cleanup.
- Added manager, CLI, and narrow trailing-status fixtures. The staged Phase 7 proof uses only an owned isolated `/tmp/p8-isolated-test-*` root and production coordinator logic to exercise Focus, Background, Reattach, Start New, Remove, End, End All confirmation behavior, retained legacy-bundle CLI listing, GUI close survival, reopen, CLI kill, exact identity cleanup, and zero residue.
- Narrowed earlier P5, P6, and P7 source guards only for the exact Phase 7 socket integration files. All other locked socket, protocol, migration, CLI, bundle, staging, and CI paths remain rejected.
- Updated `docs/features/muxy-cli.md` only after the bundle CLI byte comparison passed. It now documents attachment as workspace placement, including hidden tabs without a terminal renderer.
- The retained CLI remained 45,326 bytes with SHA-256 `e9fe05bf57067cc0bd3345bc37a09730fb44fef85e96a37d18ec92b4d4d7ac32`.

**Read-only review and fixes:**

The required single read-only Phase 7 review reported six material findings. All were verified and fixed without a second review cycle:

1. Owner-changed failed attachments exposed End but durable-owner validation made cleanup impossible. Exact daemon ownership remains mandatory, while a stale durable owner no longer blocks cleanup of that exact daemon descriptor and workspace link.
2. End could leave hidden renderer-free workspace tabs stale. Single and bulk ending now prevalidate, persist, and install workspace-tab removals around exact session termination. Ended descriptors without durable links are excluded from managed sessions.
3. `kill-session` treated a missing runtime as an empty session list. Runtime absence now returns `persistent session runtime is unavailable`, with fake-target and real isolated no-socket coverage.
4. Choosing restart-required disable hid the manager immediately. Status composition now follows `AppliedSessionMode`, not the uncommitted desired preference.
5. Accessibility labels existed only in the manager model and the status control was mouse-only. Popover rows now propagate labels into rendered accessibility text, and the status control accepts keyboard activation.
6. The original staged Start New check did not prove that the same durable tab received a replacement daemon. The staged proof now directly ends exact isolated sessions, runs production Start New and reconciliation, verifies the original tab receives a different live session ID, separately proves Remove and End semantics, and tracks the replacement daemon identities through cleanup.

**Observed failures and fixes:**

1. `cargo fmt --all && cargo test -p muxy --locked --offline session_manager --no-run` initially failed with `error[E0308]: mismatched types` because a test compared `Option<&ArcCow<'_, str>>` with `Option<&str>`. Mapping the shared label through `label.as_ref()` fixed compilation.
2. `cargo test -p muxy --locked --offline socket_kill_session_reports_a_real_missing_runtime -- --exact` passed while running zero tests because the exact module path was omitted. The corrected full-path command ran and passed one test.
3. The first `scripts/check.sh` attempt rejected the hard-coded test literal `muxy-dev.sock` under environment-policy ownership. The test now derives the socket filename from `RuntimePathPolicy`.
4. The next check rejected the entire changed socket directory under the prior P6 guard. That guard now permits only `socket/catalog.rs`, `socket/runtime.rs`, `socket/commands/mod.rs`, and `socket/commands/sessions.rs`, while rejecting every other socket path.
5. Clippy then rejected a redundant closure in the new session command. Replacing `.map(|reply| CommandResult::changed(reply))` with `.map(CommandResult::changed)` fixed it.
6. The next check reached the prior P7 Composer guard and rejected the same Phase 7 socket integration. Its exception was narrowed to those four exact files.
7. The following check reached the P5 notification guard and rejected `socket/catalog.rs`. Its exception was narrowed to that exact catalog file.
8. A later full check completed source guards, Clippy, builds, staging guards, and verifier self-tests, then encountered two unrelated timing failures in `muxy-api`: `workflow_read_phases_honor_identity_cancellation` failed while waiting for `--porcelain=1`, and `create_worktree_turns_every_setup_failure_into_a_nonrollback_warning` failed after a Git subprocess timeout while preparing a temporary worktree. The exact focused tests each passed individually, in 3.54 seconds and 0.37 seconds respectively. The unchanged full `scripts/check.sh` rerun then passed.
9. The exact Linux full-app cross-check failed before project code with exit status 101 because `freetype-sys` could not find `x86_64-linux-gnu-g++` and `psm` could not find `x86_64-linux-gnu-gcc`. The documented unsandboxed Zig-wrapper supplemental check passed with 21 existing Linux-only warnings.

**Focused verification before the final gate:**

```text
cargo fmt --all && cargo test -p muxy --locked --offline session_manager --no-run
cargo test -p muxy --locked --offline session_manager
cargo test -p muxy --locked --offline socket
cargo test -p muxy --locked --offline socket_kill_session_reports_a_real_missing_runtime -- --exact
cargo test -p muxy --locked --offline socket::commands::sessions::tests::socket_kill_session_reports_a_real_missing_runtime -- --exact
bash -n scripts/verify-p8-terminal-memory.sh
git diff --check
cargo fmt --all -- --check
cargo test -p muxy-api --locked --offline repository::ai::tests::workflow_read_phases_honor_identity_cancellation -- --exact
cargo test -p muxy-api --locked --offline worktree_lifecycle::tests::create_worktree_turns_every_setup_failure_into_a_nonrollback_warning -- --exact
```

After the type fix, the manager suite passed 9 tests and the socket suite passed 39 tests. The corrected isolated missing-runtime command passed one test. Shell syntax, formatting, and diff checks passed. Both focused `muxy-api` reruns passed.

**Exact Phase 7 gate:**

The gate was run in the plan's order:

```text
cargo test -p muxy --locked --offline session_manager
cargo test -p muxy --locked --offline socket
scripts/verify-p8-terminal-memory.sh --fixture session-manager
scripts/verify-p8-terminal-memory.sh --fixture cli-sessions
scripts/verify-p8-terminal-memory.sh --fixture status-trailing-group
scripts/check.sh
cargo check -p muxy --all-features --locked --offline --target x86_64-unknown-linux-gnu
scripts/build-app.sh debug
scripts/verify-bundle.sh target/debug/Muxy.app debug
staged_app="$(scripts/stage-test-app.sh target/debug/Muxy.app p8-phase-7)"
scripts/verify-p8-terminal-memory.sh --staged debug "$staged_app" phase-7
cmp -s Muxy/Resources/scripts/muxy-cli target/debug/Muxy.app/Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli
git diff --check
```

The manager and socket suites passed 9 and 39 tests. The fixtures reported:

```text
P8 Session Manager sections, actions, confirmations, owner validation, and stale cleanup fixture passed
P8 retained CLI session heads, exact columns, errors, permissions, and frozen catalog fixture passed
P8 Composer, resource, and session status trailing group fixture passed
```

After the documented transient and guard-integration fixes, `scripts/check.sh` passed shell and source guards, formatting, Clippy with warnings denied, workspace builds and tests, staging safety, all verifier self-tests, debug app build, helper-first signing, and bundle verification. The exact Linux cross-check produced the documented missing-GNU-compiler environmental failure.

The supplemental command was run unsandboxed:

```text
PATH="$PWD/target/test-verification/p8/toolchain:$PATH" \
AR_x86_64_unknown_linux_gnu='zig ar' \
cargo check -p muxy --all-features --locked --offline \
--target x86_64-unknown-linux-gnu
```

It passed with 21 existing Linux-only unused or dead-code warnings. The explicit debug build and strict bundle verification passed. Staging produced `/Users/saeed/Projects/muxy-2.x/target/test-verification/apps/p8-phase-7/MuxyTests.app`, and the verifier reported `P8 staged Phase 7 production manager actions, retained CLI survival, reopen, kill, and zero residue passed`. The exact CLI bundle comparison and final `git diff --check` passed.

**Unverified or intentionally deferred after Phase 7:**

- Native Linux-host app and CLI runtime behavior were not executed. The full app cross-target compiled only through the supplemental Zig wrappers after the host lacked the required GNU cross-compilers.
- Manual native and visual acceptance of popover density, narrow status layout, keyboard focus indication, screen-reader announcements, destructive confirmation presentation, and live session transitions remains unobserved. Focused tests, fixtures, and staged runtime verification cover the underlying state, actions, accessibility labels, and keyboard bindings.
- Hostile verifier matrices, final debug/release isolation, release staging, complete documentation, and roadmap closure remain assigned to Phase 8.

**Publication target:** Phase 7 commit `P8 phase 7: add session manager and CLI control` on `p8-terminal-memory-sessions`, pushed to draft PR [#1106](https://github.com/muxy-app/muxy/pull/1106), base `2.x`.
