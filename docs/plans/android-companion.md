# Android Companion — Implementation Plan

## Goal

Build an Android remote-control client for Muxy. Same scope as `MuxyMobile/`
(iOS): pair with the desktop app, browse projects/worktrees/tabs, view
terminal output, send keyboard input. Transport is the existing
`MuxyRemoteServer` WebSocket protocol on port 4865, accessed over Tailscale
(or any network the user controls).

## Decisions (confirmed)

- **Scope:** Remote control (view + input).
- **Transport:** Tailscale / VPN. Manual host entry. mDNS optional later.
- **Auth:** Pairing code (trust-on-first-use, same flow as iOS).
- **Repo:** Same Muxy repo, in a new top-level `android/` directory.
- **Q1 — Terminal emulator: Termux's `terminal-emulator` + `terminal-view`
  libraries**, vendored under `android/terminal/vendor/` (GPL-3.0,
  acceptable for this project). Native Android `View` with `Canvas` glyph
  rendering — best mobile UX on the platform: native text-selection
  handles + system selection toolbar, hardened IME handling that disables
  predictive text and fixes Samsung/Gboard quirks, native overscroll/fling
  scroll physics, fast rendering on heavy output bursts. Battle-tested in
  the most-used Android terminal. Wiring shape mirrors iOS's SwiftTerm:
  `TerminalEmulator.append(bytes, len)` for output, key handler →
  `TerminalSession` for input.
- **Q2 — Min Android SDK: API 31 (Android 12).** ~85% device coverage,
  Material 3, predictive back, cleaner IME APIs, modern window insets.
  Target SDK = latest stable.
- **Q3 — UI toolkit: Hybrid (Compose + Views).** Compose for all app
  screens (connect, project list, workspace, settings, notifications);
  drop to Android `View` for the terminal via `AndroidView { … }` since
  Termux ships a `View`. Same approach iOS uses (SwiftUI + `UIView`
  representable). No pure-XML layouts.
- **Q4 — Connection lifecycle: reconnect-on-foreground.** Clean
  disconnect when app backgrounds; auto-reconnect on foreground +
  network change. No persistent foreground service in v1. Banner UI
  surfaces "Reconnecting…" state.
- **Q5 — Push notifications: deferred.** v1 ships in-app notifications
  only, driven by the existing `notification` event stream. No FCM, no
  Google Play Services dependency, no Mac-side push code. Revisit after
  v1 ships.
- **Q6 — Distribution: GitHub Releases.** Sign release APK in CI, attach
  to GitHub releases. Users sideload. F-Droid / Play Store only if there
  is real demand later.

## Phase status legend

`[ ]` not started · `[~]` in progress · `[x]` done

---

## Phase 0 — Pre-work and decisions

**Goal:** Lock open decisions, capture protocol fixtures from a live
Mac+iOS session for use as test vectors during the protocol port.

- [ ] Capture JSON fixtures from a real iOS↔Mac session: pair, auth
      (success + 401 → pair flow), list projects, open workspace,
      `takeOverPane` → `terminalSnapshot`, live `terminalOutput` chunks,
      `terminalInput`, `terminalResize`, `terminalScroll`,
      `paneOwnershipChanged`, `themeChanged`, `notificationReceived`
      event, `getProjectLogo`. Save under
      `docs/plans/android-fixtures/*.json`. Note: fixtures contain UUIDs
      and dates — round-trip tests must use schema/shape equality or
      pre-normalize those fields, not raw byte equality
- [ ] Confirm Mac-side protocol exposes everything Android needs (audit
      `MuxyShared/MuxyProtocol.swift` and compare against MuxyMobile's
      `ConnectionManager.swift` request surface)
- [ ] Identify any Mac-side gaps — currently only one confirmed: no
      `platform` field on `ApprovedDevice` / `pairDevice` params (so the
      approval sheet can't say "Pixel 8 Pro (Android)"). `deviceName` is
      already passed and shown today (`PairingRequestCoordinator.swift`
      line 83). mDNS `_muxy._tcp.local` is not advertised — file as a
      small follow-up if we want LAN discovery

---

## Phase 1 — Android project scaffolding

**Goal:** Empty Android app builds, runs, and lives at `android/`.

- [ ] Create `android/` directory with Gradle Kotlin DSL multi-module setup
- [ ] Modules: `:app` (UI), `:protocol` (DTOs + envelope + codec),
      `:net` (WebSocket client + connection manager), `:terminal`
      (Termux libs vendor + Compose wrapper)
- [ ] Base deps: Compose BOM, kotlinx.serialization-json,
      kotlinx.coroutines, OkHttp (WebSocket), AndroidX lifecycle,
      AndroidX security-crypto, AndroidX datastore-preferences
- [ ] Min SDK 31, target SDK = latest stable, compile SDK = latest stable
- [ ] App scaffolding: `MainActivity`, theme, navigation host (Navigation
      Compose)
- [ ] `.gitignore` for Android artifacts (`build/`, `.gradle/`,
      `local.properties`, IDE files)
- [ ] Sample `local.properties.example`
- [ ] README in `android/` with build instructions (Android Studio version,
      JDK version, target/min SDK)

---

## Phase 2 — Protocol port

**Goal:** Kotlin types that round-trip the same JSON as `MuxyShared/`.

- [ ] Port envelope: `MuxyMessage`, `MuxyRequest`, `MuxyResponse`,
      `MuxyEvent` → `:protocol`. Note `MuxyMessage` wraps payload as
      `{type, payload}` — `MuxyParams`/`MuxyResult`/`MuxyEventData` use a
      different shape: `{type, value}` (see
      `MuxyShared/MuxyProtocol.swift`)
- [ ] Port enums: `MuxyMethod`, `MuxyEventKind`, error codes. Error
      codes used today: `400 invalidParams`, `401 unauthorized`
      (semantically: "device unknown — try `pairDevice` next"),
      `403 pairingDenied`, `404 notFound`, `408 pairingTimeout`,
      `500 internalError`. Skip `registerDevice` from the wire — it is a
      `MuxyMethod` value but iOS never sends it; the Mac calls its own
      delegate's `registerDevice` internally inside `finalizeAuth`
- [ ] **Event-kind ↔ data-case naming mismatches** — JSON has both an
      outer event kind and an inner data type, and they are NOT the same
      string. Decoder must handle: `workspaceChanged` → data
      `workspace`; `projectsChanged` → data `projects`;
      `notificationReceived` → data `notification`. Other pairs match
      (`tabChanged`/`tab`, `terminalOutput`/`terminalOutput`,
      `terminalSnapshot`/`terminalSnapshot`,
      `paneOwnershipChanged`/`paneOwnership`, `themeChanged`/`deviceTheme`)
- [ ] Port DTOs: `ProjectDTO`, `WorktreeDTO`, `WorkspaceDTO` (with
      `SplitNodeDTO`, `SplitBranchDTO`, `TabAreaDTO`, `TabDTO`,
      `TabKindDTO`, `SplitDirectionDTO`, `SplitPositionDTO`),
      `NotificationDTO`, `VCSStatusDTO`, `VCSBranchesDTO`,
      `VCSCreatePRResultDTO`, `TerminalOutputEventDTO`,
      `TerminalContentDTO`, `TerminalCellsDTO` (+ `TerminalCellDTO`,
      `TerminalCellFlag`), `PaneOwnerDTO`, `PaneOwnershipEventDTO`,
      `DeviceThemeEventDTO`, `PairingResultDTO`, `DeviceInfoDTO`,
      `ProjectLogoDTO`, `TabChangeEventDTO`, `ProjectIconColor`. The
      live wire uses `TerminalCellsDTO` for `getTerminalContent`
      responses; `TerminalContentDTO` is unused on the wire today but
      ships in the Swift module — port both for parity
- [ ] Port `ProtocolParams.*` request/result types
- [ ] Configure kotlinx.serialization with ISO 8601 date serializer
      matching `MuxyCodec` exactly (verify with fixtures)
- [ ] JSON round-trip tests against captured fixtures (decode, re-encode,
      assert shape/value equality after normalizing UUIDs and dates —
      raw byte equality is unreliable because key order isn't guaranteed
      and our fixtures contain randomized UUIDs/timestamps)
- [ ] Base64 helper for `Data` ↔ ByteArray. `Data` Codable on Apple
      defaults to base64 strings on the wire, so terminal bytes
      (`TerminalInputParams.bytes`, `TerminalOutputEventDTO.bytes`,
      `TerminalContentDTO.content`-as-base64-snapshot,
      `ProjectLogoDTO.pngData`) and the device `token` (32 random bytes,
      base64-encoded) are all base64 strings on Android too

---

## Phase 3 — WebSocket client + connection manager

**Goal:** Kotlin equivalent of iOS's `ConnectionManager.swift`.

- [ ] `MuxyClient` in `:net`: OkHttp `WebSocket` over `ws://host:port`
      (no TLS — see Phase 4 security note). Lifecycle (connect, close,
      error), exponential-backoff reconnect with jitter. Send/receive
      uses **text frames** (Mac uses `NWProtocolWebSocket` opcode
      `.text`, iOS uses `URLSessionWebSocketTask.send(.string)`) — JSON
      goes over text frames, not binary
- [ ] Request/response correlation via string ID (iOS uses
      `UUID().uuidString`, not numeric). Exposed as
      `suspend fun send(method, params, timeout): MuxyResponse?`. Per-
      request timeout (iOS defaults: 10s normal, 120s for `pairDevice`)
- [ ] **Fire-and-forget path for `terminalInput`** — the Mac's
      `voidMethods` set drops the response for `terminalInput`
      (`MuxyServer/MuxyRemoteServer.swift:262`). Awaiting it leaks
      pending-request entries on every keystroke. Mirror iOS's
      `sendFireAndForget` (`ConnectionManager.swift:620`)
- [ ] Event bus: `MutableSharedFlow<MuxyEvent>` per event kind, plus a
      typed `terminalOutput(paneID)` `Flow<ByteArray>` accessor that
      demuxes both `terminalOutput` AND `terminalSnapshot` events to the
      same per-pane handler (iOS pattern in
      `ConnectionManager.swift:820-823`)
- [ ] **No `subscribe`/`unsubscribe` requests are sent** — the Mac's
      handler is a no-op (`MuxyRemoteServer.swift:616-618`) and every
      authenticated client receives every event. All filtering is
      client-side via the per-pane handler map. Do NOT add per-pane
      subscribe RPCs
- [ ] Connection state machine: `Idle → Connecting → Authenticating →
      AwaitingApproval → Connected → Reconnecting → Failed`. Modeled as
      a sealed class exposed via `StateFlow`. `AwaitingApproval` is the
      window between sending `pairDevice` and the Mac user tapping
      Approve/Deny in the modal NSAlert (can take up to the 120s
      pairing timeout)
- [ ] Connection trace ring buffer (last N events, timestamped) — exposed
      to UI for the error-report sheet (mirror iOS
      `diagnosticLog`/`recordDiagnostic`)
- [ ] Reconnect strategy: ping-then-reconnect on foreground (iOS uses
      `URLSessionWebSocketTask.sendPing` to verify a stale socket before
      tearing it down — `ConnectionManager.swift:394`). On reconnect,
      replay `selectProject(activeProjectID)` so the Mac re-emits
      workspace state — pane ownership resets to Mac on disconnect
      (`RemoteServerDelegate.clientDisconnected` calls
      `releaseAll(clientID)`), so any taken-over panes will need
      re-takeover after reconnect
- [ ] Tests with OkHttp `MockWebServer`: handshake, authenticate flow
      including 401-then-pair, fire-and-forget `terminalInput`, RPC
      round-trip, event delivery (with the kind/data naming mismatch),
      reconnect

---

## Phase 4 — Pairing

**Goal:** Trust-on-first-use, matching the iOS handshake exactly.

- [ ] `DeviceCredentialsStore` (Kotlin): generate `deviceID` (UUID) +
      `token` (32 random bytes from `SecureRandom`, **base64-encoded
      string** — match iOS `DeviceCredentialsStore.generateToken` which
      returns `Data(bytes).base64EncodedString()`) on first launch.
      Persist in `EncryptedSharedPreferences` (Android Keystore-backed)
- [ ] **No client-side hashing.** The client sends the **raw** token in
      both `authenticateDevice` and `pairDevice`. The Mac hashes
      (SHA-256) for comparison inside `ApprovedDevicesStore`
      (`Muxy/Services/ApprovedDevicesStore.swift:85`). Do not pre-hash
      on Android
- [ ] Connect flow: send `authenticateDevice(deviceID, deviceName,
      token)` → on `401 unauthorized` send `pairDevice(deviceID,
      deviceName, token)` and wait up to 120s for the Mac user to
      approve in the modal NSAlert. Success result for both is
      `MuxyResult.pairing(PairingResultDTO)` carrying `clientID`,
      `deviceName`, and optional `themeFg/themeBg/themePalette`. On
      `403 pairingDenied` surface "Approval denied on Mac"
- [ ] **Security note (plaintext over `ws://`)** — there is no TLS;
      device token + all traffic travel in the clear. Safe on Tailscale
      / a trusted VPN, dangerous on open Wi-Fi. Surface this in connect
      UX (e.g. a one-time "Use only on a trusted network" notice) and
      document in the README
- [ ] Pair pending state in UI: "Awaiting approval on Mac" with cancel
      button. Note the UX constraint: pairing requires the Mac user to
      be physically at their Mac to dismiss an `NSAlert`
      (`Muxy/Services/PairingRequestCoordinator.swift:79-100`) — there
      is no asynchronous push approval today
- [ ] Forget-device action (settings) clears local credentials

---

## Phase 5 — Connect + project list UI

**Goal:** User can connect to their Mac and see the project list.

- [ ] `ConnectScreen` (Compose): host text field, port field
      (default 4865; the Mac dev build listens on 4866 —
      `MobileServerService.defaultPort = MuxyRemoteServer.defaultPort + 1`
      when `AppEnvironment.isDevelopment`, so document this for anyone
      pointing Android at a debug Mac), connect button. Persist
      last-used host/port in DataStore
- [ ] (Optional, behind feature flag) mDNS discovery via `NsdManager` for
      `_muxy._tcp.local` — only useful on LAN, not Tailscale. Show as
      suggestions, never required
- [ ] `ProjectListScreen`: render `[ProjectDTO]` from `listProjects`,
      show project color, and only call `getProjectLogo` for projects
      whose `logo` field is non-nil (iOS pattern in
      `ConnectionManager.fetchLogo` — `ProjectLogoDTO.pngData` is
      base64 PNG, decode and cache by `projectID`). Tap to navigate
- [ ] ViewModel layer: `ConnectViewModel`, `ProjectListViewModel`,
      observing `MuxyClient` flows
- [ ] Settings entry stub (server address management)

---

## Phase 6 — Terminal rendering + pane ownership (Termux libs)

**Goal:** Open a tab, see live output, type into it. The big rock.

**Protocol shape (correct mental model):**
The Mac broadcasts `terminalOutput` events to all authenticated clients
all the time, but it will **silently drop** `terminalInput`,
`terminalResize`, and `terminalScroll` from any client that does not
own the pane (`Muxy/Services/RemoteServerDelegate.swift:128-159`).
Ownership is single-writer: at any moment a pane is owned by either the
Mac or exactly one remote client. To interact:

1. Client sends `takeOverPane(paneID, cols, rows)`.
2. Mac assigns ownership, then sends a one-shot `terminalSnapshot`
   event (a `TerminalOutputEventDTO` carrying VT bytes synthesized from
   current cells via `RemoteTerminalSnapshotBuilder`) to that client
   only — this is the initial scrollback.
3. Mac broadcasts a `paneOwnershipChanged` event so all clients update
   their owner map.
4. Live updates continue as `terminalOutput` events.
5. Client sends `releasePane(paneID)` when navigating away. On
   disconnect, the Mac auto-releases all panes the client owned
   (`RemoteServerDelegate.clientDisconnected`).

`getTerminalContent` returns parsed cells (`TerminalCellsDTO`) and is
NOT used by iOS for the live terminal path — skip it for v1 unless we
later want a non-takeover read-only view. Do **not** send `subscribe` /
`unsubscribe` requests; the Mac's handler is a no-op.

- [ ] Vendor Termux libraries under `android/terminal/vendor/`:
      `terminal-emulator/` (the VT emulator core, pure Java) and
      `terminal-view/` (the Android `View` + key handling +
      selection). Pin to a specific upstream commit (Termux's main
      repo: `termux/termux-app`, modules `terminal-emulator/` and
      `terminal-view/`). Include `LICENSE` and an `UPSTREAM` file
      noting the commit
- [ ] Strip Termux-specific bits we don't need: extra-keys row (we
      ship our own accessory bar), Termux-specific styling/preferences,
      session manager (we own session lifecycle via `MuxyClient`)
- [ ] Implement a `MuxyTerminalSession` that adapts Termux's
      `TerminalSession` to our transport: outgoing bytes from the
      session's PTY-write path are intercepted and routed to
      `MuxyClient.terminalInput(paneID, bytes)` (fire-and-forget)
      instead of writing to a local FD; incoming bytes from
      `terminalOutput` AND `terminalSnapshot` events (both carry
      `TerminalOutputEventDTO.bytes`) are fed into the emulator via
      `TerminalEmulator.append(buf, len)` on the session's thread
- [ ] `MuxyTerminalView` Compose wrapper around Termux's
      `TerminalView` (AndroidView interop). Inputs: `paneID`, font
      size, theme palette, `isOwnedBySelf` flag
- [ ] **Pane ownership state in `MuxyClient`**: `paneOwners:
      Map<UUID, PaneOwnerDTO>` plus `myClientID: UUID?` set from the
      `PairingResultDTO` returned by `pairDevice`/`authenticateDevice`.
      Update from the `paneOwnershipChanged` event. Helper:
      `fun paneIsOwnedBySelf(paneID): Boolean` matching iOS
      `ConnectionManager.paneIsOwnedBySelf`
- [ ] **Take-over UX**: when the active terminal tab is not owned by
      this client, render an overlay ("Controlled on <ownerName>",
      Take Over button) over a hit-testing-disabled terminal view —
      mirror `MobileTakeOverOverlay` in
      `MuxyMobile/TerminalView.swift:111-160`. Owner name comes from
      `PaneOwnerDTO.displayName` (either `.mac(name)` or
      `.remote(deviceID, name)`)
- [ ] **Lifecycle**:
      - On view attach with known cols/rows, send
        `takeOverPane(paneID, cols, rows)` automatically (matches
        iOS `attemptAutoTakeOver`). Reset emulator first so the
        `terminalSnapshot` reply lands on a clean grid
      - The `terminalSnapshot` event will arrive as a normal
        `terminalOutput`-shaped event (`MuxyEventKind.terminalSnapshot`,
        data `terminalSnapshot(TerminalOutputEventDTO)`) — feed its
        bytes through the same emulator path as live output
      - On detach / paneID change / tab switch, send
        `releasePane(paneID)` and unhook the per-pane byte handler
      - On reconnect (after backgrounding or network drop), the
        previously taken-over panes are no longer owned by this client
        (server released on disconnect) — re-issue `takeOverPane` on
        the active pane after the workspace is re-fetched
- [ ] Resize: hook `TerminalView.onSizeChanged` → derive cols/rows
      from Termux's font metrics → send `terminalResize(paneID,
      cols, rows)` to Mac. iOS additionally re-issues `takeOverPane`
      with new cols/rows when geometry changes — match that pattern
      so the Mac PTY stays in sync (`TerminalView.swift:80-81`)
- [ ] **Scroll forwarding**: send `terminalScroll(paneID, deltaX,
      deltaY, precise)` for trackpad/mouse-wheel-style gestures the
      emulator should not consume locally (iOS uses this in
      `ConnectionManager.scrollTerminal`). Termux's mouse reporting
      handles the in-emulator case when the remote shell has mouse
      mode enabled
- [ ] **Theme comes from the Mac, not from local config**. Apply the
      palette from `PairingResultDTO` (`themeFg`, `themeBg`,
      `themePalette` — all optional UInt32 RGB values) to Termux's
      `TerminalColors`. Re-apply when a `themeChanged` event arrives
      (data `deviceTheme(DeviceThemeEventDTO)`) — iOS does this in
      `ConnectionManager.handleEvent` `case .deviceTheme`. Fall back to
      a built-in dark default when the Mac sends no theme
- [ ] **Honor terminal mode flags from `TerminalCellsDTO`** if/when we
      use `getTerminalContent`: `altScreen`, `cursorKeys` (DECCKM —
      changes arrow-key encoding), `bracketedPaste`, `focusEvent`,
      `mouseEvent` + `mouseFormat` (mouse mode + SGR/x10/etc format).
      These also propagate naturally through the VT bytes stream from
      `terminalSnapshot`/`terminalOutput`, so the emulator state should
      track them automatically — but verify against fixtures
- [ ] Native selection / clipboard: Termux's `TerminalView` provides
      Android selection handles + system toolbar — verify Copy and
      Paste hook into Android `ClipboardManager`; Paste pushes bytes
      back through `terminalInput` (respect `bracketedPaste` mode)
- [ ] Mouse mode: Termux already encodes touch as SGR mouse events
      when the remote enables mouse reporting — verify it works for
      panning in `vim` / `htop`, fix routing if needed
- [ ] IME: confirm Termux's prediction-disabling `InputConnection` is
      active so Gboard / Samsung keyboard don't double-commit
- [ ] Manual QA: run `top`, `vim`, `htop`, `tmux`. Verify cursor
      movement, 256-color, mouse mode, bracketed paste, Unicode,
      large-output throughput, native selection on phone + tablet,
      take-over from Mac↔Android↔iOS handoff, ownership overlay
      appears when another device grabs the pane

---

## Phase 7 — Mobile keyboard accessory bar

**Goal:** Custom toolbar above the soft keyboard for terminal-friendly
keys, matching iOS. (Termux's stock extra-keys row is stripped during
vendoring; we ship Muxy's own.)

- [ ] `TerminalAccessoryBar` Compose row pinned above IME via
      `WindowInsets.ime`
- [ ] Keys: D-pad (4 arrows), Esc, Tab, `~`, `|`, `/`, `-`, Ctrl, Shift,
      Alt, Cmd-as-Meta. Modifier keys arm the next keystroke (matches
      iOS). Long-press for sticky modifier
- [ ] Map armed-modifier + key to the same byte sequences MuxyMobile
      emits — capture iOS bytes for a known set of chords (Ctrl+C,
      Ctrl+D, Alt+B, etc.) and assert equality
- [ ] Hardware keyboard: rely on Termux's `TerminalView` key handling
      for the standard chords; intercept only what Muxy needs to
      override

---

## Phase 8 — Workspace tree + tab UI

**Goal:** Render the worktree → split → tab area structure from
`WorkspaceDTO` and let the user switch tabs.

- [ ] Worktree picker (sheet/dropdown), wired to `selectWorktree`
- [ ] Tab strip (horizontal, scrollable) for the active tab area, wired
      to `selectTab`
- [ ] Active tab content: `MuxyTerminalView` for terminal tabs,
      placeholder for VCS/editor/diff tabs ("Open on desktop")
- [ ] Split rendering: v1 shows only the active leaf — note this clearly
      in the UI ("Use desktop to manage splits"). Defer recursive split
      pane rendering
- [ ] Live updates from `workspace` events
- [ ] Pull-to-refresh on workspace screen as a manual escape hatch

---

## Phase 9 — Notifications

**Goal:** In-app notification list with click-to-navigate.

- [ ] Notification list screen: render from
      `listNotifications` + live `notification` events
- [ ] Unread badge on tab bar / project rows
- [ ] Tap notification → dispatch `selectProject` → `selectWorktree`
      (if needed) → `focusArea` → `selectTab`
- [ ] Mark-read on tap, swipe-to-dismiss, "mark all read"
- [ ] FCM push deferred to a future phase (out of scope for v1)

---

## Phase 10 — Connection lifecycle and resilience

**Goal:** App survives the messy realities of mobile networking.

- [ ] App-foreground / app-background hooks via `ProcessLifecycleOwner`:
      clean disconnect on background, auto-reconnect on foreground
- [ ] Network-change handling via `ConnectivityManager.NetworkCallback`:
      reconnect when network returns
- [ ] Reconnect respects exponential backoff with jitter and surfaces a
      banner ("Reconnecting…")
- [ ] No foreground service in v1. If a future user need emerges, revisit
      as an opt-in toggle with battery warning

---

## Phase 11 — Polish + shipping prep

**Goal:** v1 is presentable and supportable.

- [ ] Settings screen: server profiles (multiple Macs), terminal font +
      size, theme override, accessory-bar layout, about
- [ ] Error-report sheet that exports the connection trace + device
      info to share/save (mirror iOS)
- [ ] App icon, splash, store-listing copy stubs
- [ ] Phone + tablet layouts (responsive Compose)
- [ ] Accessibility pass: TalkBack labels on all controls, focus order,
      large-text scaling
- [ ] Manual end-to-end QA matrix: pair, unpair, multi-Mac, network
      drop, large scrollback, busy TUI, hardware keyboard

---

## Phase 12 — CI + release

**Goal:** APK builds in CI, distributable to chosen channel(s).

- [ ] GitHub Actions workflow for Android: assemble, lint, unit tests,
      instrumented tests on a single emulator
- [ ] Detekt (lint) + ktlint (format) checks; integrate into a
      `scripts/checks-android.sh` separate from the existing Swift
      `scripts/checks.sh`
- [ ] Debug signing config in repo; release signing key + password held
      in GitHub Actions secrets, applied only on tagged builds
- [ ] Tag-driven release workflow: on `vX.Y.Z-android` tag, build signed
      release APK, generate release notes, attach APK to a GitHub
      Release. No Play Store / F-Droid for v1
- [ ] Update root `README` with Android sideload instructions (download
      APK from Releases, enable "Install unknown apps" for the browser
      or file manager)
- [ ] Update `docs/architecture.md` to add an "Android App (MuxyAndroid)"
      section mirroring the existing "iOS App (MuxyMobile)" subsection

---

## Mac-side companion tasks (small, can interleave)

These may surface during Phase 0 audit or later phases:

- [ ] (Maybe) Add Bonjour `_muxy._tcp.local` advertisement to
      `MuxyRemoteServer` so LAN discovery works on both iOS and Android
- [ ] (Maybe) Extend `PairDeviceParams` / `AuthenticateDeviceParams`
      with an optional `platform` field (`deviceName` is already
      passed and already shown in the approval alert today —
      `PairingRequestCoordinator.swift:83`). Add a matching `platform`
      field to `ApprovedDevice` so `MobileSettingsView` can render
      "Pixel 8 Pro (Android)" vs "iPhone 16 Pro (iOS)"

---

## Cross-cutting non-goals for v1

- Editor / diff viewer / file tree on Android (server returns these
  tabs, app shows "Open on desktop")
- Creating or destroying splits from Android
- AI usage panel
- FCM system push
- Offline mode

## Estimated effort

Rough order-of-magnitude, single engineer, full-time:

- Phases 0–2: ~1 week
- Phases 3–4: ~1 week
- Phases 5–6: ~2 weeks (Phase 6 is the big unknown)
- Phases 7–8: ~1.5 weeks
- Phases 9–10: ~1 week
- Phases 11–12: ~1 week

Total: ~7–8 weeks for v1, plus contingency on Phase 6.
