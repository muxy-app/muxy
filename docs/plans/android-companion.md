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

- [ ] Capture JSON fixtures from a real iOS↔Mac session: pair, list
      projects, open workspace, terminal output chunks, terminal input,
      resize, notifications event. Save under
      `docs/plans/android-fixtures/*.json`
- [ ] Confirm Mac-side protocol exposes everything Android needs (audit
      `MuxyShared/MuxyProtocol.swift` and compare against MuxyMobile's
      `ConnectionManager.swift` request surface)
- [ ] Identify any Mac-side gaps (e.g. mDNS advertisement, per-device
      platform tag in `ApprovedDevicesStore`, device-name on pair so the
      approval sheet shows a friendly name) — file as separate small tasks

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
      `MuxyEvent` → `:protocol`
- [ ] Port enums: `MuxyMethod`, `MuxyEvent` event names, error codes
- [ ] Port DTOs: `ProjectDTO`, `WorktreeDTO`, `WorkspaceDTO` (with
      `SplitNodeDTO`, `TabAreaDTO`, `TabDTO`), `NotificationDTO`,
      `VCSStatusDTO`, `TerminalOutputEventDTO`, `TerminalContentDTO`,
      `ProjectIconColor`
- [ ] Port `ProtocolParams.*` request/result types
- [ ] Configure kotlinx.serialization with ISO 8601 date serializer
      matching `MuxyCodec` exactly (verify with fixtures)
- [ ] JSON round-trip tests against captured fixtures (decode, re-encode,
      assert byte-equality after key normalization)
- [ ] Base64 helper for `Data` ↔ ByteArray (terminal bytes are
      base64-encoded on the wire — confirm against fixtures)

---

## Phase 3 — WebSocket client + connection manager

**Goal:** Kotlin equivalent of iOS's `ConnectionManager.swift`.

- [ ] `MuxyClient` in `:net`: OkHttp `WebSocket`, lifecycle (connect,
      close, error), exponential-backoff reconnect with jitter
- [ ] Request/response correlation via numeric ID, exposed as
      `suspend fun send(method, params): Result`
- [ ] Event bus: `MutableSharedFlow<MuxyEvent>` per event type, plus a
      typed `terminalOutput(paneID)` `Flow<ByteArray>` accessor
- [ ] Connection state machine: `Idle → Connecting → Authenticating →
      Pairing → Ready → Reconnecting → Failed`. Modeled as a sealed class
      exposed via `StateFlow`
- [ ] Connection trace ring buffer (last N events, timestamped) — exposed
      to UI for the error-report sheet
- [ ] Subscribe / unsubscribe helpers per pane
- [ ] Tests with OkHttp `MockWebServer`: handshake, authenticate flow,
      RPC round-trip, event delivery, reconnect

---

## Phase 4 — Pairing

**Goal:** Trust-on-first-use, matching the iOS handshake exactly.

- [ ] `DeviceCredentialsStore` (Kotlin): generate `deviceID` (UUID) +
      `token` (32 random bytes via `SecureRandom`) on first launch.
      Persist in `EncryptedSharedPreferences` (Android Keystore-backed)
- [ ] SHA-256(token) helper. Verify byte-for-byte equality with iOS
      output for a fixed test vector
- [ ] Connect flow: send `authenticateDevice` → on `401` send
      `pairDevice` → poll/await approval result
- [ ] Pair pending state in UI: "Awaiting approval on Mac" with cancel
      button
- [ ] Forget-device action (settings) clears local credentials

---

## Phase 5 — Connect + project list UI

**Goal:** User can connect to their Mac and see the project list.

- [ ] `ConnectScreen` (Compose): host text field, port field
      (default 4865), connect button. Persist last-used host/port in
      DataStore
- [ ] (Optional, behind feature flag) mDNS discovery via `NsdManager` for
      `_muxy._tcp.local` — only useful on LAN, not Tailscale. Show as
      suggestions, never required
- [ ] `ProjectListScreen`: render `[ProjectDTO]` from `listProjects`,
      show project logo (via `getProjectLogo`) and color, tap to navigate
- [ ] ViewModel layer: `ConnectViewModel`, `ProjectListViewModel`,
      observing `MuxyClient` flows
- [ ] Settings entry stub (server address management)

---

## Phase 6 — Terminal rendering (Termux libs)

**Goal:** Open a tab, see live output, type into it. The big rock.

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
      `MuxyClient.terminalInput(paneID, bytes)` instead of writing to
      a local FD; incoming bytes from `terminalOutput` events are fed
      into the emulator via `TerminalEmulator.append(buf, len)` on the
      session's thread
- [ ] `MuxyTerminalView` Compose wrapper around Termux's
      `TerminalView` (AndroidView interop). Inputs: `paneID`, font
      size, theme palette
- [ ] Lifecycle: on attach send `subscribe(paneID)` +
      `getTerminalContent(paneID)` and prime the emulator with the
      scrollback bytes; on detach send `unsubscribe(paneID)` and
      release the emulator + view
- [ ] Resize: hook `TerminalView.onSizeChanged` → derive cols/rows
      from Termux's font metrics → send `terminalResize(paneID,
      cols, rows)` to Mac
- [ ] Theme: build a Termux `TerminalColors` palette from the active
      project's Ghostty config (16 ANSI + foreground/background/cursor)
      with a sane built-in dark default fallback
- [ ] Native selection / clipboard: Termux's `TerminalView` provides
      Android selection handles + system toolbar — verify Copy and
      Paste hook into Android `ClipboardManager`; Paste pushes bytes
      back through `terminalInput`
- [ ] Mouse mode: Termux already encodes touch as SGR mouse events
      when the remote enables mouse reporting — verify it works for
      panning in `vim` / `htop`, fix routing if needed
- [ ] IME: confirm Termux's prediction-disabling `InputConnection` is
      active so Gboard / Samsung keyboard don't double-commit
- [ ] Manual QA: run `top`, `vim`, `htop`, `tmux`. Verify cursor
      movement, 256-color, mouse mode, bracketed paste, Unicode,
      large-output throughput, native selection on phone + tablet

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
- [ ] (Maybe) Extend `pairDevice` params to accept a `deviceName` and
      `platform` so the approval sheet on Mac shows "Pixel 8 Pro
      (Android)" instead of a UUID
- [ ] (Maybe) Add a `platform` field to `ApprovedDevicesStore` entries
      and a small UI tweak in `MobileSettingsView` to render it

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
