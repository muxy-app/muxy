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
  libraries**, vendored under `android/terminal/vendor/`. **Licensing
  decision (confirmed): GPL-3.0 is accepted for the Android app.** This
  means the resulting Android APK is distributed under GPL-3.0 — anyone
  receiving the APK gets re-distribution rights and corresponding source
  must be made available. Action items this triggers:
  - Add a top-level `LICENSE` file at `android/LICENSE` (GPL-3.0 full
    text) plus an `UPSTREAM` file noting the pinned Termux commit
  - In-app About screen must include a "Source code" link satisfying
    GPL §6 (link to the Muxy repo and to the Termux upstream)
  - Root README / license docs must say the repo is mixed-license:
    existing Swift targets stay under the current license, while the
    Android binary and `android/` source are GPL-3.0
  - GitHub releases must link the exact source tag used for the APK,
    not just the default branch

  Native Android `View` with `Canvas` glyph rendering — best mobile UX
  on the platform: native text-selection handles + system selection
  toolbar, hardened IME handling that disables predictive text and fixes
  Samsung/Gboard quirks, native overscroll/fling scroll physics, fast
  rendering on heavy output bursts. Battle-tested in the most-used
  Android terminal. Wiring shape mirrors iOS's SwiftTerm:
  `TerminalEmulator.append(bytes, len)` for output, key handler →
  `TerminalSession` for input.
- **Q2 — Min Android SDK: API 31 (Android 12).** ~85% device coverage,
  Material 3, cleaner IME APIs, modern window insets. Support predictive
  back on Android versions where the platform exposes it. Target SDK =
  latest stable.
- **Q3 — UI toolkit: Hybrid (Compose + Views).** Compose for all app
  screens (connect, project list, workspace, settings, notifications);
  drop to Android `View` for the terminal via `AndroidView { … }` since
  Termux ships a `View`. Same approach iOS uses (SwiftUI + `UIView`
  representable). No pure-XML layouts.
- **Q4 — Connection lifecycle: reconnect-on-foreground.** Android v1
  cleanly closes the socket when the app backgrounds, then reconnects on
  foreground + network change. This is intentionally Android-specific:
  iOS keeps its socket and pings on foreground, but Android v1 has no
  persistent foreground service. **Reconnect UI is silent — match iOS's
  user-facing behavior.** Keep state visually connected while the new
  socket is being recreated, and only show a "Reconnecting…" banner if
  reconnect actually fails into the error state.
- **Q5 — Push notifications: deferred.** v1 ships an in-app notification
  list backed by `listNotifications` snapshots only. No FCM, no Google
  Play Services dependency, no live `notificationReceived` stream until
  Mac-side event emission exists. Revisit after v1 ships.
- **Q6 — Distribution: GitHub Releases.** Sign release APK in CI, attach
  to GitHub releases. Users sideload. F-Droid / Play Store only if there
  is real demand later.

## Phase status legend

`[ ]` not started · `[~]` in progress · `[x]` done

---

## Phase 0 — Pre-work and decisions

**Goal:** Lock open decisions, capture protocol fixtures from a live
Mac+iOS session for use as test vectors during the protocol port.

- [ ] Capture JSON fixtures from a real iOS↔Mac session. **Only four
      events actually fire on the wire today** — verified by grepping
      every `MuxyEvent(event: …)` and `broadcast/send` call-site in
      `Muxy/`:
      1. `paneOwnershipChanged` (broadcast — `RemoteServerDelegate.swift:40`)
      2. `themeChanged` (broadcast — `RemoteServerDelegate.swift:45`)
      3. `terminalSnapshot` (**unicast to the taker** —
         `RemoteServerDelegate.swift:214-215`)
      4. `terminalOutput` (**unicast to the pane's current owner** —
         `RemoteTerminalStreamer.swift:42-49`)

      The DTOs/event-kinds `workspaceChanged`, `projectsChanged`,
      `notificationReceived`, `tabChanged` are defined in `MuxyShared`
      and have iOS handler cases at `ConnectionManager.swift:805-825`,
      but **no Mac code path emits them**. Don't try to capture
      fixtures for those — they will be empty. Capture instead: pair,
      auth (success + 401 → pair flow), `listProjects`, `getWorkspace`
      (response, not event), `takeOverPane` → `terminalSnapshot`
      unicast, live `terminalOutput` unicast chunks, `terminalInput`
      send, `terminalResize` send, `paneOwnershipChanged` broadcast,
      `themeChanged` broadcast, `getProjectLogo`, `listNotifications`
      response. Save under `docs/plans/android-fixtures/*.json`.
      Fixtures contain UUIDs and dates — round-trip tests must use
      schema/shape equality or pre-normalize those fields, not raw
      byte equality
- [ ] Fix stale remote docs before using them as implementation
      references. `docs/remote-server.md` currently overstates
      `workspaceChanged` as authoritative and describes
      `terminalSnapshot` as `terminalCells`; current code sends
      `terminalSnapshot` as `TerminalOutputEventDTO` bytes and emits no
      workspace/project/tab/notification events. Update that doc and
      the protocol section in `docs/architecture.md` before Phase 2
- [ ] **Spike: Termux library buildability without native deps.** The
      single highest-risk item in the whole plan. Verify before Phase 6
      that Termux's `terminal-emulator/` module builds and runs as a
      pure-Java VT core (no `libtermux.so`), and that `TerminalSession`
      can be subclassed (or its PTY-write path replaced) so outgoing
      bytes route to `MuxyClient.terminalInput` instead of a local FD.
      If JNI / native lib coupling makes this hard, the whole "vendor
      Termux" approach needs reconsideration before committing to
      Phase 1's module layout
- [ ] Confirm Mac-side protocol exposes everything Android needs (audit
      `MuxyShared/MuxyProtocol.swift` and compare against MuxyMobile's
      `ConnectionManager.swift` request surface). Now that we know only
      four events fire, this audit is short — focus on RPC coverage
      (project/workspace/tab/worktree/VCS/terminal), not events
- [ ] Identify any Mac-side gaps — confirmed:
      - No `platform` field on `ApprovedDevice` / `pairDevice` params
        (so the approval sheet can't say "Pixel 8 Pro (Android)").
        `deviceName` is already passed and shown today
        (`PairingRequestCoordinator.swift:83`)
      - mDNS `_muxy._tcp.local` is not advertised — file as a small
        follow-up if we want LAN discovery
      - **`PairingRequestCoordinator` has no server-side timeout.**
        `withCheckedContinuation` waits indefinitely
        (`PairingRequestCoordinator.swift:34-42`). The 120s "pairing
        timeout" is purely a client-side give-up timer. If the client
        reconnects before the Mac user responds, it can enqueue another
        pairing request while the first alert is still pending (queue at
        line 23 is unbounded). If the user approves after the Android
        timeout but before another pair attempt, the device is stored and
        the next authenticate succeeds. File server-side timeout as a
        Mac-side follow-up
      - **`408 pairingTimeout` error code is unreachable.** Defined in
        `MuxyError` but never returned by any code path. Drop it from
        the Android error mapping
      - No protocol version/handshake field anywhere in `MuxyMessage`
        / `MuxyRequest` / `MuxyResponse`. Future protocol changes are
        silent breakage. Document as a known limitation

---

## Phase 1 — Android project scaffolding

**Goal:** Empty Android app builds, runs, and lives at `android/`.

- [ ] Create `android/` directory with Gradle Kotlin DSL multi-module setup
- [ ] Modules: `:app` (UI), `:protocol` (DTOs + envelope + codec),
      `:net` (WebSocket client + connection manager), `:terminal`
      (Termux libs vendor + Compose wrapper)
- [ ] Base deps: Compose BOM, kotlinx.serialization-json,
      kotlinx.coroutines, OkHttp (WebSocket), AndroidX lifecycle,
      AndroidX datastore-preferences. Do **not** use AndroidX
      security-crypto for new credential code; `EncryptedSharedPreferences`
      is deprecated. Use Android Keystore directly in Phase 4
- [ ] Min SDK 31, target SDK = latest stable, compile SDK = latest stable
- [ ] App scaffolding: `MainActivity`, theme, navigation host (Navigation
      Compose)
- [ ] Manifest permissions:
      - `android.permission.INTERNET` for the WebSocket itself
      - `android.permission.ACCESS_NETWORK_STATE` because Phase 11 uses
        `ConnectivityManager.NetworkCallback`
      - `android.permission.ACCESS_LOCAL_NETWORK` on API 37+ if v1 allows
        direct LAN hosts or mDNS discovery. Tailscale/VPN paths are not
        local network paths, but manual LAN IPs are local network paths
- [ ] **Cleartext traffic config (REQUIRED).** Android 9+ blocks
      plaintext sockets by default. Since transport is `ws://` (no
      TLS — see Phase 4), the app will fail to connect unless cleartext
      is explicitly allowed. Because users type arbitrary hosts/IPs,
      `network-security-config.xml` cannot dynamically scope cleartext to
      those user-supplied hosts. Pick one of two honest choices:
      (a) app-wide cleartext via `network-security-config.xml`
      `base-config cleartextTrafficPermitted="true"` plus a strong
      trusted-network warning, or (b) ship only a fixed allowlist of known
      domains and reject arbitrary manual hosts. For current Tailscale /
      manual-host scope, choose (a)
- [ ] **`android:allowBackup="false"` (REQUIRED for credential safety).**
      Default Android auto-backup can upload the stored credential
      ciphertext. The Android Keystore key will not restore with it, so
      restore can either leak stale blobs or create unrecoverable
      credentials. Either set `allowBackup="false"` on `<application>`,
      or add `data_extraction_rules.xml` + `full_backup_content.xml` that
      exclude the credential store. Document the choice in the README's
      security section
- [ ] **R8 / ProGuard rules for kotlinx.serialization.** Add or verify the
      kotlinx-serialization-recommended rules in modules that define
      `@Serializable` types, then prove release mode with Phase 13's
      installed release APK test. Do not rely on debug builds for codec
      confidence
- [ ] **OkHttp config.** `OkHttpClient.Builder().pingInterval(20, SECONDS)`
      so OkHttp self-detects dead sockets — required for the
      reconnect-on-foreground path in Phase 11 to work. Default is 0
      (disabled). Also: explicit `readTimeout(0, MILLISECONDS)` for the
      WebSocket (no read timeout on a long-lived stream)
- [ ] `.gitignore` for Android artifacts (`build/`, `.gradle/`,
      `local.properties`, IDE files)
- [ ] Sample `local.properties.example`
- [ ] README in `android/` with build instructions (Android Studio version,
      JDK version, target/min SDK), the cleartext-traffic note, and the
      "trusted network only" caveat from Phase 4

---

## Phase 2 — Protocol port

**Goal:** Kotlin types that round-trip the same JSON as `MuxyShared/`.

- [x] Port envelope: `MuxyMessage`, `MuxyRequest`, `MuxyResponse`,
      `MuxyEvent` → `:protocol`. **There are three JSON shapes — get all
      three or workspace decoding will silently fail:**
      1. `MuxyMessage` (outer) → `{type: "request"|"response"|"event",
         payload: ...}` (see `MuxyShared/MuxyMessage.swift:8-11`)
      2. `MuxyParams` / `MuxyResult` / `MuxyEventData` (inner enums) →
         `{type, value}` (see `MuxyShared/MuxyProtocol.swift`)
      3. `SplitNodeDTO` (workspace tree, recursive) → the inner key is
         **named after the type, not `value`**:
         `{type: "tabArea", tabArea: TabAreaDTO}` or
         `{type: "split", split: SplitBranchDTO}` (see
         `MuxyShared/WorkspaceDTO.swift:27-64`)
- [x] Port enums: `MuxyMethod`, `MuxyEventKind`, error codes. Error
      codes actually returned by the Mac: `400 invalidParams`,
      `401 unauthorized` (semantically: "device unknown — try
      `pairDevice` next"), `403 pairingDenied`, `404 notFound`,
      `500 internalError`. **`408 pairingTimeout` is defined in
      `MuxyError` but unreachable** — `PairingRequestCoordinator`
      never times out. Don't surface it as a distinct case on Android.
      Skip `registerDevice` from the wire — it is a `MuxyMethod` value
      but iOS never sends it; the Mac calls its own delegate's
      `registerDevice` internally inside `finalizeAuth` to populate
      `PaneOwnershipStore` with the client name (which becomes the
      `displayName` on `PaneOwnerDTO.remote(deviceID, name)` in the
      take-over overlay)
- [x] **Event-kind ↔ data-case naming mismatches** — JSON has both an
      outer event kind and an inner data type, and they are NOT the same
      string. Decoder must handle: `workspaceChanged` → data
      `workspace`; `projectsChanged` → data `projects`;
      `notificationReceived` → data `notification`. Other pairs match
      (`tabChanged`/`tab`, `terminalOutput`/`terminalOutput`,
      `terminalSnapshot`/`terminalSnapshot`,
      `paneOwnershipChanged`/`paneOwnership`, `themeChanged`/`deviceTheme`).
      Note: only the four owner-ship/theme/terminal pairs ever appear
      on the live wire — see Phase 0. Decoders must still handle the
      others for the unit tests / future use, but don't expect to see
      them at runtime
- [x] **Custom kotlinx.serialization serializers required** for the
      Swift enums-with-associated-values that don't match kotlinx's
      default polymorphic shape:
      1. `SplitNodeDTO` — wire is `{type, tabArea}` / `{type, split}`,
         not the kotlinx default `{type, value}`. Write a custom
         `KSerializer` that reads the discriminator and dispatches
      2. `PaneOwnerDTO` — Swift enum `.mac(name)` / `.remote(deviceID,
         name)`. Verify wire shape against fixtures, write custom
         serializer if it doesn't match the default
      3. `NotificationDTO.SourceDTO` — Swift enum with `.aiProvider(String)`
         associated value. Same: verify wire shape and write custom
         serializer if needed
      The default `@Serializable sealed class` with `JsonClassDiscriminator`
      will NOT produce the right JSON for these
- [x] Port DTOs: `ProjectDTO`, `WorktreeDTO`, `WorkspaceDTO` (with
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
- [x] Port `ProtocolParams.*` request/result types
- [x] Configure kotlinx.serialization to match `MuxyCodec` exactly
      (verify with fixtures):
      - ISO 8601 date serializer
      - UUID string serializer for every Swift `UUID`
      - `explicitNulls = false`, because Swift `JSONEncoder` omits nil
        optionals while kotlinx.serialization includes nulls by default
      - keep `encodeDefaults = false` unless a fixture proves Swift emits
        a default-valued field
- [x] JSON round-trip tests against captured fixtures (decode, re-encode,
      assert shape/value equality after normalizing UUIDs and dates —
      raw byte equality is unreliable because key order isn't guaranteed
      and our fixtures contain randomized UUIDs/timestamps)
- [x] Base64 serializer for `Data` ↔ ByteArray. `Data` Codable on Apple
      defaults to base64 strings on the wire, while kotlinx.serialization
      `ByteArraySerializer` encodes a JSON list of numbers by default.
      Do not use Kotlin's default `ByteArray` JSON shape. The actual
      `Data`-typed wire fields are only `TerminalInputParams.bytes` and
      `TerminalOutputEventDTO.bytes`. `ProjectLogoDTO.pngData` is a
      `String` that already contains base64 (`MuxyShared/ProtocolParams.swift:506`).
      The device `token` is also a `String` field (already base64-encoded
      client-side via `Data(bytes).base64EncodedString()`) — not a
      `Data` field. **`TerminalContentDTO.content` is a `String`, NOT
      a `Data` / base64 field** (`MuxyShared/ProtocolParams.swift:253`).
      Earlier drafts of this plan said it was base64; that was wrong

---

## Phase 3 — WebSocket client + connection manager

**Goal:** Kotlin equivalent of iOS's `ConnectionManager.swift`.

- [x] `MuxyClient` in `:net`: OkHttp `WebSocket` over `ws://host:port`
      (no TLS — see Phase 4 security note). Lifecycle (connect, close,
      error), exponential-backoff reconnect with jitter. Send/receive
      uses **text frames** (Mac uses `NWProtocolWebSocket` opcode
      `.text`, iOS uses `URLSessionWebSocketTask.send(.string)`) — JSON
      goes over text frames, not binary
- [x] Request/response correlation via string ID (iOS uses
      `UUID().uuidString`, not numeric). Exposed as
      `suspend fun send(method, params, timeout): MuxyResponse?`.
      Per-request timeouts from iOS, mirror them exactly:
      - default RPC: 10 s
      - `pairDevice`: 120 s (client-side give-up — Mac has no timeout)
      - `vcsCommit`: 60 s
      - `vcsPush`, `vcsPull`, `vcsCreatePR`: 120 s
      - `vcsSwitchBranch`, `vcsCreateBranch`: 30 s
      - `vcsAddWorktree`, `vcsRemoveWorktree`: 60 s
- [x] **Fire-and-forget path for `terminalInput`** — the Mac's
      `voidMethods` set drops the response for `terminalInput`
      (`MuxyServer/MuxyRemoteServer.swift:262`); confirmed
      `voidMethods` contains only `terminalInput`. Awaiting it leaks
      pending-request entries on every keystroke. Mirror iOS's
      `sendFireAndForget` (`ConnectionManager.swift:620`)
- [x] Event bus: `MutableSharedFlow<MuxyEvent>` per event kind, plus a
      typed `terminalOutput(paneID)` `Flow<ByteArray>` accessor that
      demuxes both `terminalOutput` AND `terminalSnapshot` events to the
      same per-pane handler (iOS pattern in
      `ConnectionManager.swift:820-823`). **Important caveat:** both
      events are unicast — `terminalOutput` is sent only to the pane's
      current owner (`RemoteTerminalStreamer.swift:42-49`),
      `terminalSnapshot` is sent only to the client that just took
      over (`RemoteServerDelegate.swift:214-215`). A non-owner gets
      no live output at all
- [x] **No `subscribe`/`unsubscribe` requests are sent** — the Mac's
      handler is a no-op (`MuxyRemoteServer.swift:616-618`). The Mac
      decides for itself which client receives what (broadcast for
      ownership/theme; unicast-to-owner for terminal bytes). Filtering
      on the client is per-pane handler dispatch, not subscription
      management. Do NOT add per-pane subscribe RPCs
- [x] **Suppress error surfacing while backgrounded.** When a send
      fails AND `isBackgrounded`, swallow the error and don't
      transition to `.error` state — iOS does this at
      `ConnectionManager.swift:719-727`. Without this, every
      foreground/background cycle that catches an in-flight request
      flashes a spurious error banner
- [x] Connection state machine: `Idle → Connecting → Authenticating →
      AwaitingApproval → Connected → Reconnecting → Failed`. Modeled as
      a sealed class exposed via `StateFlow`. `AwaitingApproval` is the
      window between sending `pairDevice` and the Mac user tapping
      Approve/Deny in the modal NSAlert (can take up to the 120s
      pairing timeout)
- [x] Connection trace ring buffer — **120 entries** (matches iOS
      `diagnosticLog`), timestamped with ISO 8601 (fractional seconds)
      via a single shared formatter. Exposed to UI for the error-report
      sheet (mirror iOS `recordDiagnostic`)
- [x] Reconnect strategy: ping-then-reconnect on foreground. iOS calls
      `URLSessionWebSocketTask.sendPing` first; if it errors, it
      tears the socket down and runs `reconnectSilently()`
      (`ConnectionManager.swift:374-394`). On Android, OkHttp's
      `pingInterval(20s)` (Phase 1) makes the listener fire on dead
      sockets automatically; the foreground hook only needs to verify
      `webSocket != null` and trigger silent reconnect if not.
      Reconnect must:
      1. Be guarded by an `isReconnecting` flag so concurrent triggers
         only fire once (iOS `ConnectionManager.swift:404-405`)
      2. Clear `paneOwners` (iOS `:407`) — server side cleared them
         on disconnect via `RemoteServerDelegate.clientDisconnected`
         → `releaseAll(clientID)`
      3. Re-authenticate, then explicitly call
         `selectProject(activeProjectID)` followed by `getWorkspace`
         (iOS does this through `selectProject` → `refreshWorkspace` at
         `ConnectionManager.swift:424-426` and `:489-534`). Raw
         `selectProject` returns only `ok`; it does not carry workspace.
         **There is no `workspaceChanged` event from the server** —
         without `getWorkspace`, the workspace is stale (deferred to
         the workspace UI layer in Phase 8 — `:net` exposes the
         primitives `selectProject` + `getWorkspace`)
      4. Any active terminal view will need to re-issue `takeOverPane`
         after reconnect (server released ownership on disconnect)
- [x] **Pane ownership reset points** — clear `paneOwners` on:
      project select (iOS `:493`), reconnect (iOS `:407`), disconnect
      (iOS `:166`). Plan only mentions disconnect originally; the
      other two matter because stale owner data renders a wrong
      "Controlled by X" overlay on the next pane mount
- [x] **Saved devices CRUD.** iOS persists `[SavedDevice]`
      (`name + host + port`, Codable list) and exposes add / remove on
      the Connect screen — not just "last-used host/port". Port the
      full list, with `add(SavedDevice)`, `remove(SavedDevice)`, and
      ordered iteration. Persist as JSON in DataStore
- [x] Tests with OkHttp `MockWebServer`: handshake, authenticate flow
      including 401-then-pair, fire-and-forget `terminalInput`, RPC
      round-trip, event delivery (with the kind/data naming mismatch),
      reconnect

---

## Phase 4 — Pairing

**Goal:** Trust-on-first-use, matching the iOS handshake exactly.

- [x] `DeviceCredentialsStore` (Kotlin): generate `deviceID` (UUID) +
      `token` (32 random bytes from `SecureRandom`, **base64-encoded
      string** — match iOS `DeviceCredentialsStore.generateToken` which
      returns `Data(bytes).base64EncodedString()`) on first launch.
      Persist with Android Keystore directly: create an AES-GCM key via
      `KeyGenerator` in `AndroidKeyStore`, encrypt `deviceID` + `token`,
      and store ciphertext + IV in a private prefs/DataStore file that is
      excluded from backup. Do not use `EncryptedSharedPreferences`; it is
      deprecated
- [x] **No client-side hashing.** The client sends the **raw** token in
      both `authenticateDevice` and `pairDevice`. The Mac hashes
      (SHA-256) for comparison inside `ApprovedDevicesStore`
      (`Muxy/Services/ApprovedDevicesStore.swift:85`). Do not pre-hash
      on Android
- [x] Connect flow: send `authenticateDevice(deviceID, deviceName,
      token)` → on `401 unauthorized` send `pairDevice(deviceID,
      deviceName, token)` and wait up to 120 s for the Mac user to
      approve in the modal NSAlert. **The 120 s is a client-side
      give-up timer only — the Mac has no timeout and its NSAlert
      blocks until tapped** (`PairingRequestCoordinator.swift:34-42`).
      If the client times out and the user later approves, the device
      is stored on the Mac but the client has already moved on — the
      next reconnect will succeed via `authenticateDevice` (no second
      pair prompt). Success result for both RPCs is
      `MuxyResult.pairing(PairingResultDTO)` carrying `clientID`,
      `deviceName`, and optional `themeFg/themeBg/themePalette`. On
      `403 pairingDenied` surface "Approval denied on Mac"
- [x] **Security note (plaintext over `ws://`)** — there is no TLS;
      device token + all traffic travel in the clear. Safe on Tailscale
      / a trusted VPN, dangerous on open Wi-Fi. Surface this in connect
      UX (e.g. a one-time "Use only on a trusted network" notice) and
      document in the README
- [x] Pair pending state in UI: "Awaiting approval on Mac" with cancel
      button. **The cancel button is local-only** — it stops the
      Android client from waiting on the response, but the Mac alert
      stays up until the user taps it. Document this in the UX so
      users know to dismiss the Mac alert manually if they cancel on
      Android. Pairing also requires the Mac user to be physically at
      their Mac to dismiss the `NSAlert`
      (`Muxy/Services/PairingRequestCoordinator.swift:79-100`) — there
      is no asynchronous push approval today
- [x] **Backup safety check.** Verify Phase 1's `allowBackup` /
      `data_extraction_rules.xml` decision actually excludes the
      credential ciphertext store holding `deviceID` + `token`.
      Manually run `adb shell bmgr backupnow` after pairing and
      inspect the resulting backup to confirm credentials are not
      included
- [x] Forget-device action (settings) clears local credentials and the
      Android Keystore key. There is no remote revoke RPC today, so the
      Mac's approved-device entry remains until the user removes it in
      Mac settings

---

## Phase 5 — Connect + project list UI

**Goal:** User can connect to their Mac and see the project list.

- [x] `ConnectScreen` (Compose): saved-devices list at top (rendered
      from the `SavedDevice` store from Phase 3), each row tap-to-
      connect, swipe-to-remove. Below the list: an "Add device" form
      with name + host text field + port field (default 4865; the Mac
      dev build listens on 4866 —
      `MobileServerService.defaultPort = MuxyRemoteServer.defaultPort + 1`
      when `AppEnvironment.isDevelopment`, so document this for anyone
      pointing Android at a debug Mac). On successful connect, persist
      the device into the saved list (matches iOS behavior)
- [ ] (Optional, behind feature flag) mDNS discovery via `NsdManager` for
      `_muxy._tcp.local` — only useful on LAN, not Tailscale. Show as
      suggestions, never required
- [x] `ProjectListScreen`: render `[ProjectDTO]` from `listProjects`,
      show project color, and only call `getProjectLogo` for projects
      whose `logo` field is non-nil (iOS pattern in
      `ConnectionManager.fetchLogo` — `ProjectLogoDTO.pngData` is
      base64 PNG, decode and cache by `projectID`). Tap to navigate
- [x] ViewModel layer: `ConnectViewModel`, `ProjectListViewModel`,
      observing `MuxyClient` flows
- [x] Settings entry stub (server address management)

---

## Phase 6 — Terminal rendering + pane ownership (Termux libs)

**Goal:** Open a tab, see live output, type into it. The big rock.

**Protocol shape (correct mental model — verified against the Mac
implementation, not the original draft of this plan):**

Ownership is **single-writer AND single-reader**: at any moment a
pane is owned by either the Mac or exactly one remote client, and
**only the owner sees live output**. There is no "spectator" or
"read-only viewer" mode in v1. Specifically:

- `terminalOutput` is **unicast to the current owner only**
  (`RemoteTerminalStreamer.swift:42-49`:
  `server?.send(event, to: clientID)`). A non-owner gets nothing.
- `terminalSnapshot` is **unicast to the client that just took over**
  (`RemoteServerDelegate.swift:214-215`).
- `paneOwnershipChanged` is broadcast to all authenticated clients,
  so non-owners can render the "Controlled by X" overlay correctly.
- The Mac silently drops `terminalInput`, `terminalResize`, and
  `terminalScroll` from any client that does not own the pane
  (`RemoteServerDelegate.swift:128-159`).

To interact:

1. Client sends `takeOverPane(paneID, cols, rows)`.
2. Mac assigns ownership and unicasts a one-shot `terminalSnapshot`
   event (a `TerminalOutputEventDTO` carrying VT bytes synthesized
   from current cells via `RemoteTerminalSnapshotBuilder`) to that
   client only — this is the initial visible grid snapshot. It is not
   historical scrollback.
3. Mac broadcasts a `paneOwnershipChanged` event so all clients
   update their owner map.
4. Mac attaches `RemoteTerminalStreamer` to the pane's surface and
   begins unicasting live `terminalOutput` events to the new owner.
5. Client sends `releasePane(paneID)` when navigating away. On
   disconnect, the Mac auto-releases all panes the client owned
   (`RemoteServerDelegate.clientDisconnected` →
   `releaseAll(clientID)`).

**Implication for UX:** A user opening the workspace sees the tab
list but no live output until they tap "Take Over" on a pane.
Auto-take-over on view attach (already in the plan, kept below) is
what makes this feel seamless. Don't promise live preview without
take-over.

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
      cols, rows)` to Mac. **Do NOT re-issue `takeOverPane` on resize
      — iOS doesn't.** iOS only auto-takes-over once per `paneID`
      (guarded by `autoTakenPaneID == paneID` at
      `MuxyMobile/TerminalView.swift:99-108`); subsequent geometry
      changes only send `terminalResize`. Replicate the once-per-paneID
      guard exactly — set `autoTakenPaneID = paneID` BEFORE the take-
      over coroutine launches so concurrent triggers (first size
      report racing onAppear) only fire the RPC once
- [ ] **Scroll forwarding — `terminalScroll` is currently unused on
      iOS.** The function exists at
      `MuxyMobile/ConnectionManager.swift:641` but has zero callers in
      `MuxyMobile/`. iOS handles wheel/pan gestures by injecting SGR
      mouse-button-4/5 bytes through `terminalInput` when
      `mouseMode != .off` (see `MuxyMobile/TerminalView.swift:462-499`).
      **For v1, mirror iOS exactly: do not call `terminalScroll`. Use
      Termux's existing mouse reporting (it encodes touch as SGR mouse
      events when the remote enables mouse mode), and skip the RPC.**
      Revisit only if Android needs scroll behavior beyond what iOS does
      today
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

Reference implementation: `MuxyMobile/TerminalView.swift:685-1014`
(`TerminalAccessoryView`, `ModifierKeyButton`, `ModifierPickerView`,
`DPadControl`).

- [ ] `TerminalAccessoryBar` Compose row pinned above IME via
      `WindowInsets.ime`
- [ ] **Key set (full iOS parity)**:
      - `Esc`, `Tab`
      - **Paste** button — reads Android `ClipboardManager` and sends
        bytes through `terminalInput` (must respect `bracketedPaste`
        mode when active). iOS at `TerminalView.swift:709`
      - **Copy** button — copies current selection to system clipboard;
        disabled when no selection. iOS at `TerminalView.swift:710`
      - `~`, `|`, `/`, `-`
      - **Modifier key** (Ctrl/Shift/Alt/Cmd-as-Meta) with two gestures:
        - **Tap** = arm/disarm the active modifier for the next
          keystroke (one-shot, then auto-clears)
        - **Long-press** = open a small popover picker that lets the
          user *change* which modifier is the active one (Ctrl ↔ Shift
          ↔ Alt ↔ Cmd). NOT "sticky" — the picker just changes which
          modifier the tap arms. iOS at `TerminalView.swift:885-998`
          (`handleLongPress` + `ModifierPickerView`)
      - **Keyboard hide/show toggle** — `keyboard.chevron.compact.down`
        when visible, `keyboard` when hidden. Swaps the IME for an
        empty placeholder view to dismiss. iOS at
        `TerminalView.swift:765-776` and `MuxySwiftTermView.toggleKeyboard`
        at `TerminalView.swift:410-416`
      - **D-pad** — analog thumb stick (not 4 buttons), with deadzone
        and **auto-repeat: 300 ms initial delay, then 60 ms cadence**.
        iOS at `TerminalView.swift:1207-1315` (`DPadControl`,
        `startRepeating`)
- [ ] Modifier transform table — match iOS `MuxySwiftTermView.transform`
      at `TerminalView.swift:501-523` exactly:
      - Ctrl + `a..z`/`A..Z` → control byte (`a` → 0x01, etc.)
      - Ctrl + space → 0x00
      - Shift + letter → uppercase
      - Alt + text → ESC prefix (`\u{1B}` + text)
      - Cmd + text → text passthrough (Cmd-as-Meta is decorative on
        mobile)
- [ ] Map armed-modifier + key to the same byte sequences MuxyMobile
      emits — capture iOS bytes for a known set of chords (Ctrl+C,
      Ctrl+D, Alt+B, arrows, etc.) and assert equality
- [ ] Hardware keyboard: rely on Termux's `TerminalView` key handling
      for the standard chords; intercept only what Muxy needs to
      override. Test pass against hardware keyboard before signing off
      Phase 7 (real Bluetooth keyboard, DeX desktop mode if available)

---

## Phase 8 — Workspace tree + tab UI

**Goal:** Render the worktree → split → tab area structure from
`WorkspaceDTO` and let the user switch tabs.

- [ ] Tab picker (Compose `DropdownMenu` from a toolbar icon) — mirror
      iOS `RemoteWorkspaceView.tabPicker` at
      `MuxyMobile/RemoteWorkspaceView.swift:95-127`. Lists every tab
      across every area, plus a "New Terminal" item that calls
      `createTab`. **Not** a horizontal strip — iOS uses a Menu and we
      should match
- [ ] Active tab content router: `MuxyTerminalView` for `.terminal`
      tabs (with a `paneID`); for `.vcs` / `.editor` / `.diffViewer`
      kinds, render an "Open on desktop" placeholder. VCS gets a
      first-class sheet (Phase 9) — the tab placeholder is just for
      when the user lands on a VCS-kind tab in the workspace
- [ ] Split rendering: v1 shows only the focused area's active tab —
      iOS does the same (`RemoteWorkspaceView.swift:20-29`). Show
      "Use desktop to manage splits" hint. Defer recursive split pane
      rendering
- [ ] **VCS toolbar button** in the workspace top bar — opens the VCS
      sheet (Phase 9). Disabled when no active project. Mirrors iOS
      `vcsButton` at `MuxyMobile/RemoteWorkspaceView.swift:60-68`
- [ ] **Workspace sync — there is no live `workspaceChanged` event.**
      Confirmed: no Mac code path emits one. The workspace updates
      only via:
      1. An explicit `getWorkspace(activeProjectID)` after this client
         triggers a workspace-changing RPC. Raw `selectProject`,
         `selectWorktree`, `selectTab`, and `focusArea` responses are
         `ok`, not workspace payloads; `createTab` returns only the new
         tab. The Android wrapper layer must refresh workspace after
         `selectProject` / `selectWorktree` / `createTab` / `closeTab` /
         `selectTab` / `focusArea` / `splitArea` / `closeArea`
      2. Explicit user-triggered refresh via pull-to-refresh
      3. Reconnect, which re-issues `selectProject` and then
         `getWorkspace` (Phase 3)
      Tab/worktree changes made by the Mac itself or by *another*
      remote device are **invisible** to this client until a refresh.
      Document this in the UI ("Pull to refresh — workspace updates
      from other devices won't appear automatically") rather than
      promising live sync we can't deliver
- [ ] Pull-to-refresh on workspace screen — **primary sync mechanism,
      not an escape hatch.** Calls `getWorkspace(activeProjectID)` and
      replaces the local `WorkspaceDTO`. If the project or worktree no
      longer exists, surface a recoverable error and return to the project
      picker

---

## Phase 9 — Source Control (VCS)

**Goal:** Full Git workflow against the active project — match iOS's
VCS sheet, not just placeholders. iOS exposes Source Control as a
**top-level toolbar sheet from the workspace** (not a tab), reachable
from any project context. We mirror that.

Reference iOS files:

- `MuxyMobile/VCSView.swift` — status sheet (stage/unstage/discard,
  commit, push, pull, PR display)
- `MuxyMobile/BranchesSheet.swift` — list, switch, create branch
- `MuxyMobile/WorktreesSheet.swift` — list, add, remove, switch worktree
- `MuxyMobile/CreatePRSheet.swift` — full PR creation flow
- `MuxyMobile/ConnectionManager+VCS.swift` — RPC wrappers

Underlying RPCs and DTOs are already ported in Phase 2
(`getVCSStatus`, `vcsStageFiles`, `vcsUnstageFiles`, `vcsDiscardFiles`,
`vcsCommit`, `vcsPush`, `vcsPull`, `vcsListBranches`, `vcsSwitchBranch`,
`vcsCreateBranch`, `vcsCreatePR`, `vcsAddWorktree`, `vcsRemoveWorktree`,
plus `selectWorktree`).

### 9.1 — Connection manager VCS extension

- [ ] Kotlin extension on `MuxyClient` matching the iOS surface in
      `ConnectionManager+VCS.swift`:
      - `suspend fun fetchVCSStatus(projectID): VCSStatusDTO?`
      - `suspend fun stageFiles / unstageFiles / discardFiles`
        (`discardFiles` takes both `paths` and `untrackedPaths` —
        iOS-side splits at `VCSView.swift:340-353`; preserve that
        split because the Mac-side Git ops differ for tracked vs
        untracked)
      - `suspend fun vcsCommit(projectID, message, stageAll)` —
        timeout 60 s
      - `suspend fun vcsPush(projectID)` — timeout 120 s; surface
        `noUpstreamBranch` failure path (Mac auto-fallbacks to
        `pushSetUpstream` in `RemoteServerDelegate.vcsPush:336-345`,
        no client-side handling needed)
      - `suspend fun vcsPull(projectID)` — timeout 120 s
      - `suspend fun listBranches` — default 10 s
      - `suspend fun switchBranch(projectID, branch)` — timeout 30 s
      - `suspend fun createBranch(projectID, name)` — timeout 30 s
        (Mac creates and switches in one op —
        `RemoteServerDelegate.vcsCreateBranch:389-393`)
      - `suspend fun createPullRequest(projectID, title, body,
        baseBranch, draft): VCSCreatePRResultDTO` — timeout 120 s
      - `suspend fun addWorktree(...)` — timeout 60 s; calls
        `refreshWorktrees` after success
      - `suspend fun removeWorktree(...)` — timeout 60 s; calls
        `refreshWorktrees` after success
      - `suspend fun selectWorktree(...)` — calls `refreshWorkspace`
        after success (iOS `ConnectionManager+VCS.swift:122-126`)
- [ ] Throwing wrapper helper to convert response errors into a sealed
      `VCSClientError` (timeout, server(message), unexpectedResponse) —
      mirror iOS at `ConnectionManager+VCS.swift:128-154`

### 9.2 — Source Control sheet (the equivalent of iOS `VCSView`)

- [ ] Compose modal sheet, opened from the workspace top-bar VCS button
- [ ] Sections, in order (matches iOS):
      1. **Summary** — current branch, ahead/behind counts (icons:
         `arrow.up`/`arrow.down`), pull-request link if `pullRequest`
         non-null, Pull / Push action buttons (Push disabled when
         `aheadCount == 0 && hasUpstream`)
      2. **Staged files** — count header with "Unstage All" action;
         row swipe = Unstage
      3. **Changes** — count header with "Stage All" action; row
         swipe = Stage / Discard (Discard splits paths vs
         untrackedPaths based on `GitFileDTO.isUntracked`)
      4. **Clean** — green checkmark "Working tree clean" when both
         lists empty
      5. **Commit** — `TextField` (multiline 2…5 lines), Commit
         button disabled on empty/whitespace-only message; calls
         `vcsCommit(stageAll: false)`
      6. **Error row** — when an in-flight op fails
- [ ] `StatusBadge` per file row — A / M / D / R / C / U / `!`, color
      coded (added/untracked = green, modified/renamed/copied = orange,
      deleted = red, unmerged = purple). Mirrors iOS
      `VCSView.swift:404-439`
- [ ] In-flight tracking: per-action `Set<String>` so multiple
      operations can be in flight without crashing the UI; mirror iOS
      `inFlight` at `VCSView.swift:13`
- [ ] Pull-to-refresh on the status list
- [ ] Top-bar overflow menu: Branches, Worktrees, Create Pull Request
      (the PR option is hidden when `status.pullRequest` is non-null)
- [ ] Theming: respect `deviceTheme` exactly like iOS — every row uses
      `themeFg.opacity(0.06)` for backgrounds, accent = `themeFg`.
      Mirrors iOS at `VCSView.swift:391-401`

### 9.3 — Branches sheet

- [ ] Modal sheet opened from VCS sheet overflow menu
- [ ] List local branches with checkmark on current; tap = switch (no-op
      if same as current)
- [ ] `+` button in toolbar opens an alert/dialog with a single text
      field and Create button → calls `createBranch` (creates and
      switches, per Mac at
      `RemoteServerDelegate.vcsCreateBranch:389-393`)
- [ ] Per-row spinner while the switch RPC is in flight; close sheet on
      success and call `onChange()` so the parent VCS view refreshes

### 9.4 — Worktrees sheet

- [ ] Modal sheet opened from VCS sheet overflow menu
- [ ] List worktrees from `connection.projectWorktrees[projectID]`,
      green checkmark on the active worktree (compare against
      `workspace.worktreeID`)
- [ ] Tap non-active row = `selectWorktree` → workspace refresh →
      dismiss
- [ ] Swipe-to-remove only when `worktree.canBeRemoved && !isActive`
- [ ] `+` button opens an Add Worktree form:
      - Name field
      - Branch source toggle: "New Branch" vs "Existing"
      - For existing: dropdown of `listBranches.locals`
      - For new: text field
      - Submit calls `addWorktree(createBranch: !useExistingBranch)`
      - Disable submit until name + branch are non-empty

### 9.5 — Create PR sheet

- [ ] Modal sheet opened from VCS sheet overflow menu, only when
      `status.pullRequest == nil`
- [ ] Fields: Base branch (default = `status.defaultBranch`, editable),
      Title, Body (multi-line 4…10), Draft toggle
- [ ] Disable Create until title is non-empty
- [ ] On success: open the returned PR URL in the system browser
      (Compose `LocalUriHandler` / `Intent.ACTION_VIEW`), then call
      `onCreated()` and dismiss. iOS does this at
      `CreatePRSheet.swift:96-100`
- [ ] On failure: show the error string in the sheet, do not dismiss

### 9.6 — VCS QA checklist

- [ ] Stage / unstage / discard one file; staged-all / unstaged-all
- [ ] Commit with empty message disabled; commit success clears field
- [ ] Push when upstream missing succeeds (Mac auto-`pushSetUpstream`)
- [ ] Pull merge / fast-forward / conflict surface a sensible error
      string
- [ ] Switch branch with dirty tree (Mac error must surface to user)
- [ ] Create branch with invalid name (Mac error string surfaces)
- [ ] Add worktree, switch to it, remove non-primary worktree, attempt
      to remove primary (must be blocked)
- [ ] Create PR for a branch without a pushed upstream (Mac auto-pushes
      first per `RemoteServerDelegate.vcsCreatePR:395-427`)
- [ ] Create PR with default base = `defaultBranch`
- [ ] PR link in summary opens in browser

---

## Phase 10 — Notifications

**Goal:** In-app notification list with click-to-navigate.

**Scope note — this is NEW functionality, not parity, AND there are
no live notification events on the wire.** Two facts:

1. iOS does NOT have a notifications UI today: the `notifications`
   array on `ConnectionManager` exists
   (`MuxyMobile/ConnectionManager.swift:811-813`) but is never
   displayed anywhere. iOS also does not call `listNotifications` —
   only the DemoBackend stub does.
2. **The `notificationReceived` event never fires from the real
   Mac server.** The handler case is wired up on iOS but there is
   no Mac code path that emits `MuxyEvent(event: .notificationReceived, ...)`.
   Confirmed by exhaustive grep: the only events the Mac actually
   sends are `paneOwnershipChanged`, `themeChanged`, `terminalSnapshot`,
   `terminalOutput`.

So Android's notification screen can ONLY be backed by `listNotifications`
RPC. There is no live push to merge in. Decide between two strategies:

- **Snapshot-only (simplest):** Fetch on screen open + pull-to-refresh.
  No live updates while the screen is open.
- **Periodic poll:** Fetch on screen open, then re-fetch every 30 s
  while the screen is in foreground.

Pick snapshot-only for v1; revisit if users complain.

Android shipping a notifications screen is a deliberate
platform-ahead-of-iOS choice; flag in the planning notes that real
live-push needs the Mac to start emitting `notificationReceived`
events (separate Mac-side follow-up).

- [ ] Notification list screen: call `listNotifications` on screen open
      and on pull-to-refresh. Sort by timestamp desc. **Do NOT wire up
      a "merge live events on top" path** — that event never fires
- [ ] Unread badge on tab bar / project rows — driven by the snapshot
      fetched on screen open; staleness is acceptable for v1
- [ ] Tap notification → dispatch `selectProject` → `selectWorktree`
      (if needed) → `focusArea` → `selectTab`, then call
      `getWorkspace` through the wrapper layer. If any referenced
      project/worktree/area/tab no longer exists, show "This notification
      points to a closed tab" and leave the user on the notification list
- [ ] Mark-read on tap by calling `markNotificationRead`. Do not ship
      swipe-to-dismiss or "mark all read" in v1; the Mac has local
      `NotificationStore.remove` / `markAllAsRead` helpers but exposes no
      remote RPC for them today
- [ ] FCM push deferred to a future phase (out of scope for v1).
      Live in-app push deferred too, since it requires Mac-side work
      to actually emit `notificationReceived` events first

---

## Phase 11 — Connection lifecycle and resilience

**Goal:** App survives the messy realities of mobile networking.

- [ ] App-foreground / app-background hooks via `ProcessLifecycleOwner`:
      clean disconnect on background, auto-reconnect on foreground. This
      is an Android-specific battery/lifecycle choice, not exact iOS
      parity. iOS keeps the socket around and pings on foreground; Android
      v1 closes because there is no foreground service
- [ ] Network-change handling via `ConnectivityManager.NetworkCallback`:
      reconnect when network returns. Requires
      `ACCESS_NETWORK_STATE`; unregister callbacks when the connection
      manager is disposed
- [ ] Reconnect respects exponential backoff with jitter. **Silent by
      default — keep state `Connected` while the new socket is being
      established (mirrors iOS `reconnectSilently`).** Only flip into a
      visible "Reconnecting…" banner if reconnect transitions to the
      error state, not for every fast foreground reconnect. After a
      successful reconnect, re-issue `takeOverPane` for the active
      pane (server cleared ownership on disconnect)
- [ ] **Process death recovery.** Android can kill and later restore
      the app process. After process death, in-memory state
      (`paneOwners`, `myClientID`, terminal byte handlers, the
      WebSocket itself, the `WorkspaceDTO`) is gone. On cold start
      after restoration:
      1. Read and decrypt `deviceID` + `token` from the Android
         Keystore-backed credential store (durable unless the app data or
         Keystore key is cleared)
      2. Read last-active host/port + project/worktree from DataStore
         if we want to deep-link back into the workspace
      3. Re-run the full connect → authenticate → selectProject flow
      Document which navigation state survives via `SavedStateHandle`
      (route + scroll positions) vs. which gets re-derived from the
      RPC layer (`WorkspaceDTO`, projects list)
- [ ] **Aggressive-OEM expectation note.** Samsung / Xiaomi / OnePlus
      / Huawei battery optimizations kill backgrounded apps faster
      than stock Android, and may delay the `NetworkCallback` /
      foreground-reconnect path by tens of seconds. This is a known
      limitation of "no foreground service" — surface it in the README
      ("If reconnect is slow on your phone, whitelist Muxy in
      Settings → Battery → App optimization"), don't try to fix it
      with code in v1
- [ ] No foreground service in v1. If a future user need emerges, revisit
      as an opt-in toggle with battery warning

---

## Phase 12 — Polish + shipping prep

**Goal:** v1 is presentable and supportable.

- [ ] Settings screen — match iOS parity (Android is iOS minus
      Demo Mode): Use Nerd Font toggle, font size stepper (8…24),
      About (version + build), Forget Device action. **iOS ships a
      Demo Mode toggle (`MuxyMobile/SettingsSheet.swift:39-45`) —
      Android is intentionally NOT porting it** (personal project, no
      app-store screenshots to seed). Saved devices list lives on
      the Connect screen, not Settings, so "multiple Macs" is already
      covered there. No theme override and no accessory-bar layout
      customization — iOS doesn't have these and shipping config
      Android-only diverges from iOS for no reason
- [ ] Error-report sheet that exports the connection trace + device
      info to share/save — mirror iOS `ConnectionIssueDetailsView`
      (`MuxyMobile/ContentView.swift:94-137`). Concrete pieces: a
      monospaced scrollable text area, a Copy button (uses
      `ClipboardManager`), an Android `Intent.ACTION_SEND` share
      button (Android equivalent of iOS `ShareLink`), and the body
      content built from: ISO 8601 timestamps with fractional seconds,
      last 25 diagnostic lines from the 120-entry ring buffer (Phase 3),
      version + build, connection state, target host:port, last
      request method/ID, response error code
- [ ] **Connect-state intermediate screens.** iOS routes `ContentView`
      based on `connection.state` and renders distinct screens for
      `.disconnected` (ConnectView), `.connecting` (ConnectingView),
      `.awaitingApproval` (AwaitingApprovalView), `.connected`
      (ProjectPickerView), `.error` (ErrorView with Retry / Debug Info
      / Disconnect buttons). Android's navigation host should likewise
      have explicit destinations / screens for each — not just a
      single screen with a banner. Especially `awaitingApproval` needs
      its own screen with the cancel button and the local-only-cancel
      caveat from Phase 4
- [ ] App icon, splash, store-listing copy stubs
- [ ] Phone + tablet layouts (responsive Compose)
- [ ] Accessibility pass: TalkBack labels on all controls, focus order,
      large-text scaling
- [ ] Manual end-to-end QA matrix: pair, unpair, multi-Mac, network
      drop, large scrollback, busy TUI, hardware keyboard

---

## Phase 13 — CI + release

**Goal:** APK builds in CI, distributable to chosen channel(s).

- [ ] GitHub Actions workflow for Android: assemble, lint, unit tests,
      instrumented tests on a single emulator
- [ ] Detekt (lint) + ktlint (format) checks; integrate into a
      `scripts/checks-android.sh` separate from the existing Swift
      `scripts/checks.sh`
- [ ] Debug signing config in repo; release signing key + password held
      in GitHub Actions secrets, applied only on tagged builds. Use
      APK Signature Scheme v2 + v3 (for key rotation later)
- [ ] **Verify R8 release build produces a working APK.** Phase 1
      added the kotlinx.serialization keep rules; this task is the
      end-to-end check: assemble release, install, pair, open a tab,
      take over, type into terminal, submit a VCS action. Catches
      missed `-keep` rules early (the failure mode is
      `SerializationException` at runtime with an obfuscated class
      name, which is hard to triage post-release)
- [ ] **Upload R8 mapping file to GitHub Release** alongside the APK
      so we can de-obfuscate stack traces from any user-supplied error
      report
- [ ] Tag-driven release workflow: on `vX.Y.Z-android` tag, build signed
      release APK, generate release notes, attach APK + mapping.txt to
      a GitHub Release. No Play Store / F-Droid for v1
- [ ] Update root `README` with Android sideload instructions (download
      APK from Releases, enable "Install unknown apps" for the browser
      or file manager) plus the trusted-network requirement and the
      auto-backup-disabled note from Phase 1. Also update the license
      section to call out the mixed-license repo boundary: root Swift app
      remains under the current license, Android APK/source is GPL-3.0
      because of vendored Termux terminal code
- [ ] Update `docs/architecture.md` to add an "Android App (MuxyAndroid)"
      section mirroring the existing "iOS App (MuxyMobile)" subsection
      and correct the protocol event section if Phase 0 has not already
      done so
- [ ] Update `docs/remote-server.md` if Phase 0 has not already done so:
      `workspaceChanged` is not emitted, `terminalSnapshot` carries
      `TerminalOutputEventDTO` bytes, and notification live events do not
      exist today

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
- [ ] **Pairing timeout on the Mac side.** Today
      `PairingRequestCoordinator` waits forever via
      `withCheckedContinuation` (`PairingRequestCoordinator.swift:34-42`).
      A client that gives up after 120 s leaves a queued NSAlert that
      will pop on the next reconnect, possibly stacking. Add a
      server-side timeout (e.g. 120 s matching the client) that
      auto-denies and emits the unused `408 pairingTimeout` error code.
      Without this, Android clients on flaky networks can produce a
      stack of pending Mac alerts
- [ ] **Live workspace events.** Today no `workspaceChanged`,
      `projectsChanged`, `notificationReceived`, or `tabChanged`
      events fire from the Mac, so iOS and Android cannot see changes
      made by other devices or by the Mac itself without a manual
      refresh. Adding event broadcasts on the relevant `appState`
      mutations would unlock real multi-device sync. iOS already has
      handler cases for these (`ConnectionManager.swift:805-825`); the
      missing piece is server-side emission. Out of scope for v1, but
      file as the highest-leverage Mac-side follow-up
- [ ] **Remote notification mutation RPCs.** Android v1 only supports
      `listNotifications` and `markNotificationRead` because those are
      the only notification RPCs exposed today. If users need
      swipe-to-dismiss or "mark all read", add `dismissNotification` and
      `markAllNotificationsRead` to `MuxyShared`, route them through
      `MuxyRemoteServer`, and cover them in remote routing tests before
      wiring Android UI actions
- [ ] **Protocol versioning.** No version field in `MuxyMessage` /
      `MuxyRequest` / `MuxyResponse`. Adding one (and a min-version
      check on connect) would let us evolve the protocol without
      silently breaking older clients. Out of scope for v1

---

## Cross-cutting non-goals for v1

- Editor / diff viewer / file tree on Android (server returns these
  tabs, app shows "Open on desktop")
- Creating or destroying splits from Android
- AI usage panel
- FCM system push
- Offline mode
- Demo mode (iOS has it; intentionally not ported — this is a personal
  project, no app-store screenshots to seed)
- `terminalScroll` RPC (defined in protocol, unused on iOS today —
  see Phase 6 note)
- Spectator / read-only viewer for panes the user doesn't own. The
  protocol unicasts `terminalOutput` to the owner only — supporting a
  spectator mode requires either repeated `getTerminalContent` polls
  or new Mac-side broadcast logic. Take-over remains the only way to
  see a pane's output in v1
- Live multi-device workspace sync. Without server-side emission of
  `workspaceChanged` / `projectsChanged` / `tabChanged` /
  `notificationReceived`, changes from another device require manual
  refresh on Android. Listed as a Mac-side follow-up above
- Live notification push within the app. Same root cause —
  `notificationReceived` is never emitted by the Mac today

## Estimated effort

Rough order-of-magnitude, single engineer, full-time:

- Phase 0: ~3–4 days (now includes the Termux buildability spike,
  which is non-trivial — if it fails we re-plan Phase 6)
- Phases 1–2: ~4–5 days (custom kotlinx serializers for SplitNodeDTO
  / PaneOwnerDTO / NotificationDTO.SourceDTO are real work)
- Phases 3–4: ~1 week
- Phases 5–6: ~2 weeks (Phase 6 is still the big unknown even after
  the Phase 0 spike — the spike answers "can we build it" but not
  "does it integrate cleanly with our event loop")
- Phases 7–8: ~1.5 weeks
- Phase 9 (VCS): ~1.5 weeks (five sheets + RPC layer)
- Phases 10–11: ~1 week (Phase 11 now includes process-death
  recovery and aggressive-OEM testing)
- Phases 12–13: ~1 week

Total: ~9.5–10.5 weeks for v1, plus contingency on Phase 6's
integration unknowns.
