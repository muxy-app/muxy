# Muxy 2.x — Full-Picture Migration Plan (Swift → Rust/GPUI)

## Context

Muxy 1.x is a Swift/SwiftUI macOS app — far more than a terminal: terminal emulator (libghostty) + project/worktree manager + JS extension platform + embedded browser + mobile WebSocket server + AI-agent hook system + tmux-like persistent sessions. The Rust + GPUI rewrite is now in this repository on the `2.x` branch, with the 8-crate workspace at the branch root and the Swift implementation retained alongside it as the parity reference. The architecture refactor is **fully complete** and mechanically enforced by `scripts/check.sh`. The old phase plans are consumed or superseded; this document is the single roadmap for finishing the rewrite. It has passed one adversarial review pass; corrections from that review are folded in.

**Goal:** Muxy 2.0 for macOS replaces the Swift app in `/Applications`. Release performs one bounded, source-preserving import into `~/.muxy`, then Rust owns its storage. Debug owns `~/.muxy-dev`. Linux comes after, while this roadmap keeps it compiling and launching.

## Decisions of Record (from interview)

| # | Decision |
|---|---|
| 1 | **Full parity** — 2.0 ships every Swift subsystem: extensions, browser, mobile server, AI hooks, persistent sessions, quick terminal, composer, voice, backup, CLI. |
| 2 | **Dev/prod isolation** mirrored from Swift — added early, essential for development. |
| 3 | **Webview:** direct WKWebView via objc2, hosted through the existing `native_compositor`. |
| 4 | **Extension host:** out-of-process Rust binary using **system JavaScriptCore** — exact engine parity with 1.x extensions. |
| 5 | **Persistent sessions + idle offline freeing:** both rebuilt (both default **off** in 1.x — verified). The Rust daemon is clean Rust on a private `MXS2` protocol in its own versioned socket namespace, not wire or path compatible with Swift. |
| 6 | **Updates:** Sparkle is NOT carried over. 2.x ships a unified Rust-native updater (Mac now, Linux later). Bundle id stays `com.muxy.app`; existing users upgrade by manual download-and-replace. |
| 7 | **CLI:** keep the bash wrapper untouched — "the installed CLI just works" is an acceptance test of the socket server's fidelity. |
| 8 | **Sentry: dropped entirely.** Local diagnostics (logs, profiler, Diagnostics menu, export) stay. |
| 9 | **Transparency/vibrancy:** window-level transparency+blur via GPUI honoring the existing settings; per-region NSVisualEffectView vibrancy is not reproduced. |
| 10 | **Linux (this roadmap):** app must compile, open, and show working chrome with the terminal pane as placeholder. Nothing more. |
| 11 | **Parity first, polish opportunistic** — no dedicated redesign phases. |
| 12 | **Dogfood-first ordering** — enablers → daily-driver parity → platform → outer ring → release machinery. |
| 13 | **Repo workflow:** the active rewrite repository is `~/Projects/muxy-2.x` on branch `2.x`. The Rust workspace lives at the branch root alongside the retained Swift parity reference. Feature branches PR into `2.x`; `~/Projects/muxy` remains the 1.x working copy. |
| 14 | **Release gate:** beta-channel soak + full parity checklist (details in Verification). |
| 15 | **Storage cutover:** release imports the retained Swift profile once into `~/.muxy`; debug uses `~/.muxy-dev`; existing Rust data wins and the Swift source remains untouched. |

## Open decisions (surfaced by adversarial review — resolve at owning phase's planning)

| Phase | Decision |
|---|---|
| P10 | Fate of the **in-process** command-script runner (`ExtensionScriptRunner`, a second JSC embed inside the app with synchronous `__muxyDispatch`): keep in-process (breaks "JSC only in host" rule, adds JIT entitlement to main binary) vs move to the host process (changes command-script semantics). |
| P14 | 1.x latent bug: `config-export` is missing from `verbNames` so `muxy config export` gets no reply in 1.x. Reproduce or fix. |
| P15 | 1.x sunset specifics: contents of the final 1.x release / frozen appcast, beta-channel (rolling `beta-channel` tag) cutover, and whether the Homebrew tap continues. |

## Current State — Already Done

**P0 repo migration is complete:** the active rewrite now lives on the `2.x` branch in `~/Projects/muxy-2.x`; the Rust workspace is at the repository root, the Swift implementation remains available in the same branch for parity work, and local quality gates remain in `scripts/check.sh`. CI is intentionally deferred to P15 before release.

The refactored workspace (all enforced by `scripts/check.sh` — comment ban, crate-boundary gates, fmt/clippy/tests/bundle):

```mermaid
flowchart TD
    subgraph app["crates/muxy (app binary)"]
        VIEWS["views/ (window, workspace, settings, overlays, omnibox, picker)"]
        GLUE["state / keymap / command / terminal glue / native_compositor"]
    end
    UI["muxy-ui\nGPUI kit: theme, icons, controls,\ntext_input, scrollbar"]
    TERM["muxy-terminal\nbackend trait, search, scrollbar,\nconfirmations, ghostty NSView host"]
    API["muxy-api\ngit, worktrees(read), truth, ide,\nlayouts, watcher, picker"]
    CORE["muxy-core\nprefs, settings catalog, shortcuts,\nstores, workspace tree, workspace_store"]
    HOST["ghostty-host\nsafe wrapper, RuntimeEvent channel"]
    SYS["ghostty-sys\nbindgen FFI → GhosttyKit.xcframework"]

    app --> UI
    app --> TERM
    app --> API
    API --> CORE
    TERM --> CORE
    TERM --> HOST
    HOST --> SYS
    app --> CORE
```

**Working today** (~43.6k LOC, 303 unit tests):
- Terminal stack: libghostty FFI → host → NSView (IME, CJK fonts, mouse, pressure) → GPUI compositing; shell launch with startup commands; OSC titles/PWD/progress; search; overlay scrollbar; clipboard/paste/close confirmations.
- Tabs/splits: full two-level tree (top-level tab groups + per-group split trees), drag/dock/reorder, pin/color/icon/rename, maximize, directional focus.
- Persistence uses Rust-owned profile roots. Release imports the current allowlist once from `~/Library/Application Support/Muxy` into `~/.muxy`; debug uses `~/.muxy-dev`; staged tests inject ignored roots. Existing destination data wins, the Swift source is never modified, and terminal migration outcomes never inspect it again. Preferences use private atomic `preferences.json`; only an eligible pending macOS release migration reads the production `NSUserDefaults` suite through Foundation.
- Projects/groups/worktrees (read + switch), git truth off-thread, FS watcher, project picker, omnibox (6 scopes), repo layouts (`.muxy/layouts/*`), 490 bundled themes + paired light/dark, keybindings (54 of 1.x's 68 bindable actions modelled + unmodelled passthrough — parity audit needed), chorded command shortcuts + editor UI, settings modal (16 categories + JSON editor), native menu bar, app bundling/signing scripts (ad-hoc, single-binary only).

## Gap Map — 1.x feature vs 2.x status

| Subsystem | Status in Rust |
|---|---|
| Terminal emulation, search, scrollbar, confirmations | ✅ done (CJK temp-conf machinery needs parity audit) |
| Tabs/splits/workspace persistence | ✅ done |
| Projects/groups/picker/omnibox/layouts/themes/keybindings/settings UI | ✅ done (some inert buttons; later feature parity audits remain) |
| One-way Swift profile migration and Rust-owned storage | ✅ P2.5 complete |
| Worktree **create/remove**, path templates, setup/teardown hooks | ❌ missing (read/switch only) |
| Titlebar buttons, welcome New Tab, sort, nav arrows, misc inert settings actions | ❌ inert |
| Git UI (changes/branch/PR popovers, commit/push/pull, gh, AI commit/PR text) | ❌ missing |
| Notifications (toast/panel/desktop/sounds/navigation, OSC 9/777) | ❌ missing |
| Quick Terminal (global hotkey, panel) | ⚠️ implemented; manual native acceptance pending |
| Composer / rich input (+ drafts, attachments, broadcast) | ⚠️ implemented panel-only; manual native acceptance pending |
| Idle terminal surface freeing | 🚧 P8 implemented; manual acceptance found a recovery defect |
| Persistent tab-owned sessions (`muxy-session-v2` daemon + attach) | 🚧 P8 implemented; manual acceptance found a recovery defect |
| `muxy.sock` server + verb dispatcher (3 entry surfaces) + CLI | ✅ P2 complete |
| Extension platform (host, manifests, permissions, consent, audit, marketplace, surfaces) | ❌ missing |
| Embedded browser (WKWebView, profiles, history, automation, cookie import) | ❌ missing |
| AI agent hooks (muxy-hook, installer for 11 providers, agent detection, agents layout) | ❌ missing |
| Remote/SSH workspaces + remote devices (incl. legacy SSH-workspace migration) | ❌ read-only store files |
| Mobile server (WebSocket, Bonjour, pairing, 53 RPCs) | ❌ settings only |
| Voice / dictation | ❌ missing |
| Backup/restore (`.muxy` archive) | ❌ inert buttons |
| Updater + 1.x sunset, What's New, tips, Diagnostics menu, URL scheme, doc type | ❌ missing |
| Localization (extension-provided languages, catalog) | ❌ missing |
| Drag-and-drop of files/paths (terminal, sidebar, composer) | ⚠️ implemented; manual Finder acceptance pending |
| Quit confirmation, terminateLater cleanup, relaunch-in-place | ❌ missing |
| Login-shell PATH hydration (provider/CLI discovery depends on it) | ❌ missing |
| install-skills + bundled skills + starter kits | ❌ missing |
| Deferred startup ordering, TCC usage strings, tooltip/press-and-hold defaults | ❌ missing |
| Transparency/blur, app layouts (`tabFocused`/`agentsFocused` refinements) | ❌ partial (projectFocused chrome only) |
| Dev/prod isolation | ✅ P1 contracts and settings complete; P2 socket coexistence proven; session, hook, and mobile runtime proof remains in P8/P11/P12 |
| Release signing/notarization/DMG, multi-binary bundle | ❌ ad-hoc, single binary |

Note: 1.x has **no** menu-bar extra/NSStatusItem and **no** onboarding flow — none should be assumed.

## Roadmap

```mermaid
flowchart LR
    subgraph A["Stage A — Foundations"]
        P0["P0 Repo migration ✓\n2.x branch + local gates"]
        P1["P1 Dev/prod isolation ✓\ncontracts + settings"]
        P2["P2 muxy.sock server +\ndispatcher core + CLI compat ✓"]
        P25["P2.5 One-way Swift import\nRust-owned profile roots"]
    end
    subgraph B["Stage B — Daily driver"]
        P3["P3 Chrome wiring +\nworktree create/hooks"]
        P4["P4 Git / VCS UI"]
        P5["P5 Notifications"]
        P6["P6 Quick Terminal"]
        P7["P7 Composer"]
        P8["P8 Idle freeing +\npersistent sessions"]
    end
    subgraph C["Stage C — Platform"]
        P9["P9 WKWebView foundation\n+ embedded browser"]
        P10["P10 Extension platform"]
    end
    subgraph D["Stage D — Outer ring"]
        P11["P11 AI agent hooks"]
        P12["P12 Remote/SSH + mobile"]
        P13["P13 Voice"]
        P14["P14 Backup/restore"]
    end
    subgraph E["Stage E — Release"]
        P15["P15 CI + release machinery\n(updater, 1.x sunset, notarization)"]
        P16["P16 Beta soak +\nparity audit → 2.0"]
    end
    P0 --> P1 --> P2 --> P25 --> B
    P2 --> P10
    B --> C
    P9 --> P10
    C --> D
    P11 --> P16
    D --> E
    P15 --> P16
```

Each phase gets its own detailed plan (grill-me per task) before implementation. Scopes below define boundaries and acceptance, not implementation detail.

### Stage A — Foundations

**P0 — Repo migration, workflow & local quality gates. COMPLETE.**
The active rewrite repository is `~/Projects/muxy-2.x` on branch `2.x`. The 8-crate Rust workspace is at the repository root alongside the retained Swift implementation, which remains the parity reference while the rewrite proceeds. Feature branches target `2.x`; `~/Projects/muxy` remains the 1.x working copy. Local enforcement is provided by `scripts/check.sh`, `scripts/build-app.sh`, and `scripts/verify-bundle.sh`. New phases extend crate-boundary and bundle gates when they add crates or executables. The existing 1.x workflows remain in the repository, while Rust macOS/Linux CI is deliberately deferred to P15 so it lands before release.
*Acceptance: complete.* The active repository and `2.x` workflow are established, the Rust workspace builds from the branch root, and local quality gates are available.

**P1 — Dev/prod isolation. COMPLETE.**
`muxy-core::environment` owns compile-time mode selection, storage roots, socket/session/hook names, mobile keys and defaults, and typed provider-config mutation policy. Debug uses `com.muxy.dev`, display name `Muxy Dev`, and `~/.muxy-dev`; release uses `com.muxy.app`, display name `Muxy`, and `~/.muxy`. Isolated acceptance runs permanently reuse `com.muxy.tests` and injected roots beneath `target/test-verification`. Development foreign-provider mutation policy permits only explicit AI Notifications toggle and Refresh actions; current P1 controls write only Muxy preferences, Refresh remains inert, and Test is non-mutating. P8 upgrades session roots to a Rust-only versioned namespace.
*Acceptance: complete.* Bundle identity, file storage, Ghostty configuration, defaults ownership, runtime endpoint names, and mobile namespaces are isolated by mode. Runtime feature implementation remains assigned to P8 sessions, P11 hooks/provider mutators, and P12 mobile.

**P2 — Socket server + dispatcher core + CLI compat. COMPLETE.**
Consume `RuntimePathPolicy::main_socket_path` with caller-local build selection, bind the selected socket, export it to panes, and prove debug Rust coexists with the production Swift socket. Reproduce the real `NotificationSocketServer` protocol — **newline-delimited pipe-separated text** with base64-nested JSON fields (not NDJSON); CLI command replies are raw text terminated by a NUL byte then close; server→client `invoke` RPC with 15s timeout; modal push channels; 128 KiB message cap; no protocol versioning; subscription allowlist enforced only for identified extensions; the in-flight cap (8) applies to every session, while the 100-drop disconnect is extension-scoped. **Hook-envelope handling is structural to the server**: every unidentified line gets an `AgentHookProtocol` v3 JSON parse attempt + JSON ack, 256-entry dedup ring, PID→pane resolution — this lands here (P11 adds the installer/binary, not the server semantics). Document the three verb surfaces distinctly: `SocketCommandHandler` pipe verbs, `MuxyAPIDispatcher` (146 dispatched cases; 167 accepted names including 35 legacy aliases), and the verbs implemented outside the dispatcher (`panes.split`, `tabs.rename/close/move`, lifecycle). Implement in P2 the verbs backed by existing functionality; later phases own their verbs (36 `browser.*` → P9, `create-worktree` → P3, `config-export|import` → P14). Persistent sessions have no public CLI verbs. Bundle contract: ship `muxy-cli` at the literal legacy path `Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli` (installed shims hardcode it); note the CLI's default socket is the **prod** path (dev relies on `MUXY_SOCKET_PATH` from panes) and `open-project` uses `muxy://open` + `open -b com.muxy.app`. Terminal env contract exported into panes: `MUXY_PANE_ID`, `MUXY_PROJECT_ID`, `MUXY_WORKTREE_ID`, `MUXY_SOCKET_PATH` (`MUXY_HOOK_BIN`/`MUXY_HOOK_SCRIPT` land in P11 — verify the claude wrapper tolerates their absence, else stub the staging dir here).
*Acceptance: complete.* All 33 P2 wire heads, path-open, raw hook, and raw notification pass through the installed 1.x shim and byte-identical nested CLI against an isolated staged Rust app. Synthetic extension sessions cover transport mechanics, the common gates pass, and the live production Swift socket remains unchanged.

**P2.5 — One-way Swift profile migration. COMPLETE.**
Release imports only current Rust-consumed paths from `~/Library/Application Support/Muxy` into `~/.muxy`, preserving the Swift source and every preexisting destination entry. Directory merges are recursive and do not follow symlinks or non-regular entries. Portable `preferences.json` replaces normal `NSUserDefaults` ownership; a narrow macOS reader imports only the approved production-suite keys during an eligible pending migration. Missing source, completed, and abandoned outcomes are terminal. First failure blocks startup and retries once; second failure records abandonment and continues without deleting imported data. Debug and ordinary tests never inspect the production source.
*Acceptance: complete.* Focused migration and preference tests, Linux-neutral core compilation, 47 retained Swift tests, staged synthetic debug/release launches, source hashes, retry and abandonment assertions, review remediation, and all shared gates pass.

### Stage B — Daily-driver parity

**P3 — Chrome wiring + worktree lifecycle. COMPLETE.**
Wire every inert control that has a backing feature: titlebar split/new-tab buttons, nav arrows, layout menu, welcome New Tab, workspace sort, sidebar footer, status-bar buttons (new-browser-tab button wires in P9 when browser exists). Worktree **create/remove** with `{branch}`/`{project-name}`/`{base-dir}` path templates, `worktree-checkouts/` management, `.muxy/worktree.json` + `~/.config/muxy/worktree.json` setup/teardown hooks with the approval/hash-check flow, 5-min budget, and `MUXY_*` env contract. Includes the CLI `create-worktree` verb.
*Acceptance: complete.* Chrome, sorting, navigation, native and CLI worktree creation/removal, setup/teardown hooks, bounded Git processes, failure handling, and installed-wrapper parity are verified. All 34 implemented wire heads pass through the retained byte-identical CLI, debug and release isolated launches close normally without shared-profile mutation, and the shared gates and post-implementation audit are green.
**P4 — Git/VCS UI. COMPLETE.** Changes/branch/PR popovers, stage/unstage/discard, commit, push/pull, repo status bar, `gh` integration, AI-generated commit messages and PR text. Login-shell PATH hydration lands here (git/gh/provider discovery depends on it).

*Acceptance: complete.* Selected-worktree repository identity, native status controls and popovers, safe Git and GitHub mutations, repository AI workflows, shared cancellation and mutation lanes, portable watcher/environment seams, provider privacy bounds, and isolated debug/release artifacts are verified. The post-implementation audit and shared gates are green; interactive visual acceptance remains user-owned.
**P5 — Notifications. COMPLETE.** OSC 9/777 + socket sources (hook source already flows via P2's server; extensions later) into one store: toast (4 positions), panel with read state, macOS UserNotifications when backgrounded, 14 named system sounds + None (via `NSSound(named:)`, nothing bundled), 2-s coalescing, click-to-navigate, `notifications.json` cap 200.

*Acceptance: complete.* Portable capped history, typed socket/hook and OSC ingress, delivery/coalescing policy, toast and panel presentation, unread synchronization, stable-ID navigation, target-gated UserNotifications/NSSound edges, persistence lifecycle, Linux core portability, CLI compatibility, and isolated debug/release lifecycle verification are green. Native authorization/banner/click behavior, sound audibility, and rendered visual/accessibility acceptance remain user-owned.
**P6 — Quick Terminal. IMPLEMENTED; MANUAL NATIVE ACCEPTANCE PENDING.** Global hotkey (Carbon `RegisterEventHotKey` + double-Shift with Input Monitoring), slide-out panel window, persistent home shell, size/transparency/blur settings, `quick-terminal-shortcut.json` conflict validation.
**P7 — Composer. IMPLEMENTED PANEL-ONLY; MANUAL NATIVE ACCEPTANCE PENDING.** `⌘I` on macOS and `Alt+I` elsewhere toggle one in-window panel that docks right/bottom, resizes, pins to displace the workspace, or floats as an in-window overlay. The multiline editor, image/file attachments (`RichInputImages/` plus reference sweep), pane broadcast, per-worktree drafts (`rich-input-drafts.json`), transactional Return/no-Return submission, editor typography, native pasteboard adapter, and shared parser-backed Composer/terminal/sidebar path drops are implemented. There is no standalone Composer window or modal. Automated debug/release staged lifecycle and exact-byte proof are complete; physical Finder/clipboard delivery, real TUI image interpretation, visual/focus/accessibility quality, and Linux-host launch remain manual or later-platform acceptance.
**P8 — Terminal memory features. IMPLEMENTED; NOT COMPLETE.**
(a) Idle terminal freeing destroys and recreates the in-app Ghostty surface. Ordinary terminals wake with a fresh shell in their latest directory and no old scrollback; persistent terminals reconnect and replay bounded output. Eligibility matches retained focus/visibility, running-command, OSC 133, alternate-screen, remote-exclusion, and one-shot startup-command behavior. Settings apply live and default off. Worktree-wide offline aggregation remains P10.
(b) Persistent sessions exist only for restart/crash continuity. Every local session is owned by its stable terminal-tab UUID, every recoverable tab reattaches automatically, and closing the tab terminates the complete session first. Recovery keeps layout and distinguishes unreachable **Reconnect** from daemon-confirmed missing **Start Fresh**. Enable and destructive disable require a native Restart Now/Cancel transaction and both default off. There is no tabless workflow, monitoring UI, manual reassignment, or public session CLI.
(c) `muxy-session-v2` is clean Rust and uses a private `MXS2` bounded protocol, 256 KiB replay, versioned selected-profile socket namespace, same-UID peers, singleton locking, bounded queues/connections, full OS-session process reaping, and five-shell integration. It is not wire compatible with Swift. First production 2.x launch makes one bounded, terminally recorded legacy kill-all request without changing the Swift profile.
The Phase 1 precondition exposes raw data, cell/alternate-screen reads, foreground PID, raw input, and occlusion through safe backend-neutral seams. Resource sampling, its setting, and any CPU/RAM status UI are removed.
*Acceptance: not yet met.* The private protocol, bundled daemon, tab-owned recovery, restart transactions, and live idle freeing are implemented, the shared gates pass, and the fresh post-implementation audit is closed with every Medium-or-higher finding fixed. Automated proof covers the shared gates, the complete debug staged matrix, and release launch/close, daemon harness, and recovery, all with injected roots and unchanged production profiles. **Manual acceptance then failed on the first real enable:** every staged recovery case begins with the daemon already running and a session already established, so no automated case covers the first launch after enabling, when no daemon exists yet. An established tab in that state is reported as daemon-confirmed missing and offered Start Fresh instead of reconnecting. P8 stays open until that path is fixed, covered by an automated case, and re-verified by manual acceptance.
**Stage B extras outside P8:** general quit confirmation, general terminate-later cleanup, general relaunch infrastructure, deferred-startup ordering, press-and-hold registration, and tooltip defaults.
*Stage B exit:* you live on the Rust app daily.

### Stage C — The platform

**P9 — WKWebView foundation + embedded browser.**
objc2 WKWebView host through `native_compositor`, behind a webview trait. The hardest part is **reparenting**: one WKWebView surviving moves across splits/tabs with AppKit first-responder handoff (1.x `ReparentingNSViewBroker`) — the vertical spike must cover reparenting + focus, not just embedding. Per-profile data stores with the exact 1.x mapping: the `…-00B0` sentinel maps to `WKWebsiteDataStore.default()`, **not** `forIdentifier:` (getting this wrong loses every user's default-profile cookies). Browser tabs/panes, address bar + suggestions + search engines, history + favicons, find bar, start page, zoom, error page, inspector (note: 1.x uses private SPI — decide fidelity), Chrome cookie import (v10/v11 only; v20 App-Bound Encryption silently fails in 1.x — document, don't "fix" silently), and the 36 `browser.*` verbs across their two wire shapes. Parity includes reproducing 1.x's absences (no JS dialogs, file upload, downloads, HTTP auth).
**P10 — Extension platform.**
Host: 1.x uses the Obj-C **`JSContext`** API; the Rust host on the JSC **C API** is a from-scratch bridge (class definitions, value protection/GC rooting, bidirectional JSON marshalling, exception handling) — cost it as such, and spike it first. Same signing identity, host-specific entitlements plist (JIT trio). Resolve the open decision on the **in-process** `ExtensionScriptRunner` (second JSC embed, dedicated CFRunLoop threads, synchronous `__muxyDispatch`). Manifest = `package.json` with a `muxy` key in `~/.config/muxy/extensions/<name>/` (`dist/` root when present; no `name@version` directory scheme — that's marketplace-side). All contribution points; 25 permissions; 21 events; runtime consent = 11 gated verbs × choices × matchers with 60s timeout (orthogonal to permissions), persisted grants, **rolling** audit log (1 MiB cap → 256 KiB trim); crash-restart policy (5 attempts/30s); extension update flow; dev-path live reload; localization catalog (extension-provided languages); per-extension KV storage (1MiB/value, 5MiB/store); `window.muxy` bridge injection; marketplace + install-by-URL + Load Unpacked + starter kits; rolling logs + debug console (`⌘\``); extension settings routes + shortcut registration. Note the host socket client is single-outstanding-request/no-IDs, and background scripts reach ~40 verbs + `git.*`/`browser.*` — not the full dispatcher.

### Stage D — Outer ring

**P11 — AI agent hooks.** `muxy-hook` Rust binary (protocol v3 client side, 1MiB stdin cap, 400ms budget) invokes caller-local `build_mode!` and the hook-specific default-socket policy. A non-empty inherited `MUXY_SOCKET_PATH` wins; empty or absent values use that policy. Hook resources stage through the selected `RuntimePathPolicy` directory. Build the declarative installer/verifier/repairer for all 11 providers (exact JSON shapes into foreign configs, self-write hashing, FSEvents watch, rate limiting, obsolete cleanup incl. OpenCode legacy-plugin removal), foreground-process detection fallback (4s/30s grace), agents-focused sidebar layout, provider Test/Refresh, `MUXY_HOOK_BIN`/`MUXY_HOOK_SCRIPT` env vars. Every foreign-config mutator requires and revalidates a current-mode permit. In development, only the explicit AI Notifications toggle and Refresh handlers may mint permits; a mechanical call-site allowlist and negative tests enforce this. Byte-compare staged shims and JSON against 1.x output. Server-side envelope handling already exists from P2.
**P12 — Remote/SSH + mobile.** Remote devices store (write side) + the **legacy SSH-workspace migration** (`project-groups.json` inline SSH → `remote-devices.json` — data-moving, must exist before any user's first 2.x launch), SSH workspaces/projects/worktrees, remote file browsing, attachment uploads over control sockets; WebSocket server consuming the P1 mobile keys, debug port 4866, production port 4865, and development force-on policy; prove both endpoints coexist; Bonjour `_muxy._tcp`, QR pairing (`muxy://pair`), SHA-256 token approvals, per-pane takeover, scrollback replay caps, 53 RPC methods.
**P13 — Voice.** SFSpeechRecognizer dictation in composer + legacy status-bar recorder (insert into focused control, auto-Return), AVCaptureSession metering, language catalog. Includes the permanently-retained legacy voice shortcut action.
**P14 — Backup/restore.** `.muxy` zip (ditto, manifest schemaVersion 1, exact file/dir set), export sanitization (SSH env vars, approved devices), pre-import snapshot + rollback, CLI `muxy config export|import` (resolve the 1.x `config-export` reply bug decision), settings Backup buttons, `install-skills` + bundled SKILL.md files. Import restores Muxy provider preferences without development foreign-config mutation. Any importer-triggered install, repair, uninstall, cleanup, or reconciliation requests the denied import-source authorization; negative tests and the P11 mutation-seam audit are required.

### Stage E — Release

**P15 — CI, release machinery + 1.x sunset.**
Before any 2.x release, stand up Rust CI on macOS and Linux. The macOS runner executes `scripts/check.sh`; the Linux guardrail runs per-package `cargo check --target x86_64-unknown-linux-gnu` for the headless crates and app while **excluding `ghostty-sys`/`ghostty-host`** because `ghostty-sys` build.rs panics off macOS, then performs a launch check. Burn down known Linux-launch gaps such as blank SF Symbols in `muxy-ui` and `/usr/bin/open` shell-outs. Scope the retained 1.x workflows so beta/release triggers and `BETA_VERSION` automation cannot affect `2.x`. CI must be green on both platforms before release work can pass its gate.

Unified Rust updater (feed on GitHub Releases, stable+beta channels honoring `muxy.update.channel` + `SUAutomaticallyUpdate`/`SUEnableAutomaticChecks` keys, signature verification, atomic install/relaunch, Gatekeeper/translocation-safe). **1.x sunset design (critical):** 1.x auto-updates silently from `releases/latest/download/appcast-{arch}.xml` and a rolling `beta-channel` tag — publishing 2.0 releases naively either silently auto-installs 2.0 onto all 1.x users (contradicting decision 6) or 404s their update checks forever. Ship a final 1.x release with a frozen appcast/announcement path, cut the beta channel over deliberately, and decide the Homebrew tap. Developer ID signing + hardened runtime + per-binary entitlements (JIT trio for JSC host; audio/network/apple-events; the 13 terminal-child TCC usage strings) + notarized DMG, **multi-binary bundle** (`muxy-session-v2`, `muxy-hook`, extension host in `Contents/MacOS`, each signed/stripped) + full resource inventory (terminfo, shell-integration ×5 shells, ghostty-overrides, ProviderIcons, skills, starter-kits, muxy-cli at the legacy bundle path); remove `SentryDSN` injection from release scripts (`muxy.sentry.consent` remains a preserved-unknown key, no consent prompt); `muxy://` URL scheme + `.muxy` doc type + `LSMultipleInstancesProhibited` + launch-arg guard (first positional `/`/`~` arg opens a project); What's New modal; tips card; Diagnostics menu + logs — no Sentry; CLI install/migration flow; **all ≥13 documented 1.x migrations** reproduced: ghostty seed (✅ done), paired-theme, composer-voice keybinding, quick-terminal blur, one bounded legacy session cleanup, extension `enabled` migration, CLI wrapper version, decode-with-defaults everywhere, legacy SSH workspaces (P12), `command-shortcuts.json` bare-array shape, `legacyExtraTabs` layouts, OpenCode legacy plugin removal (P11), legacy voice shortcut retention (P13).
**P16 — Beta soak + parity audit → 2.0.** See Verification.

## New components on the current architecture

The refactored layering (ARCHITECTURE.md + `scripts/check.sh` gates) is law for all new work: headless logic in headless crates, GPUI only in `muxy`/`muxy-ui`, platform code behind `cfg` with neutral APIs, new executables as new workspace crates. Swift's target split maps onto new crates:

```mermaid
flowchart TD
    subgraph existing["Existing crates"]
        muxy["muxy (app)"]
        ui["muxy-ui"]
        term["muxy-terminal"]
        api["muxy-api"]
        core["muxy-core"]
    end
    subgraph new["New crates"]
        proto["muxy-proto (P2/P8)\napp pipe/NUL framing, hook protocol,\nprivate Rust session protocol"]
        session["muxy-session-v2 (P8, binary)\nPTY daemon + private attach client"]
        exth["muxy-extension-host (P10, binary)\nsystem JSC embed (C API)"]
        hook["muxy-hook (P11, binary)\ntiny socket client"]
        web["muxy-web (P9)\nwebview trait + WKWebView impl\n(mac-gated, like muxy-terminal/ghostty)"]
    end
    muxy --> proto
    session --> proto
    session --> core
    exth --> proto
    hook --> proto
    hook --> core
    muxy --> web
```

Placement rules per phase:
- **P2 socket server:** wire types + the real framing (pipe-separated lines, NUL-terminated CLI replies, base64-JSON fields) in `muxy-proto`; the unix-socket listener as a headless transport (requests/replies over channels, no GPUI); the verb dispatcher lives in the app (it needs `AppState`, like `MuxyAPIDispatcher`), delegating real work to `muxy-core`/`muxy-api`.
- **P5 notifications / P14 backup:** models + stores in `muxy-core`; macOS delivery (UserNotifications, `ditto`) at cfg-gated edges.
- **P6 quick terminal / P13 voice:** windows/UI in `muxy`; Carbon hotkey, Speech, AVCapture as small mac-gated modules with neutral APIs.
- **P8 terminal memory:** framing and messages in `muxy-proto`; offline and process policy headless in `muxy-terminal`; raw output/cell state at the `ghostty-host` edge; session daemon as `muxy-session-v2`; app-owned tab recovery, setting transaction, and free/recreate lifecycle in `muxy`. Process inspection is mac-gated and there is no resource sampling.
- **P9 webview:** `muxy-web` mirrors the `muxy-terminal` shape — headless handle trait + signals, WKWebView NSView impl mac-gated (incl. the reparenting broker), app-side adapter adds `element()`; `views/` never names NSView types (check.sh gate extends to it).
- **P10 extensions:** manifest/permissions/consent/audit models headless (shared by app and host via `muxy-proto`); host-process JSC embedding in `muxy-extension-host`; the in-process command-script runner's placement follows the P10 open decision.
- **P15 updater:** headless feed-parse/verify crate or `muxy-api` module; platform install steps cfg-gated.
- Every phase that adds a crate extends `scripts/check.sh` boundary gates, the bundle scripts (`build-app.sh`/`verify-bundle.sh` are currently single-binary), and ARCHITECTURE.md in the same PR.

## Cross-cutting parity contracts (apply to every phase)

- **Formats:** Foundation date = f64 seconds since 2001-01-01 where retained formats require it; uppercase UUIDs; explicit JSON shape per current Rust store; private data uses atomic destination-side temporary files and file modes 0600/0700 where applicable.
- **Migration preservation:** the one-way importer copies allowlisted App Support files without parsing or reserializing them. Existing Rust files and extension data win, and the retained Swift source is never modified or deleted.
- **Sentinel UUIDs:** Home project `…-0001`, default browser profile `…-00B0` (→ `WKWebsiteDataStore.default()`), derived remote-Home IDs.
- **Settings:** normal preference reads and writes use portable `preferences.json`; `settings.json` remains the user-facing settings mirror; `NSUserDefaults` is migration-only.
- **Decoding:** every store decodes with defaults semantics appropriate to its current model; keybindings merge with defaults and avoid conflicts; malformed optional entries do not block unrelated state.
- **Rollback:** the retained Swift profile remains untouched. Rust does not write back to Swift storage or promise that Swift can consume Rust-owned `~/.muxy` data.

## Risk register

| Risk | Mitigation |
|---|---|
| Swift profile migration loses or overwrites user state | Allowlisted source-preserving copy, destination-wins merge, versioned terminal state, two-attempt cap, unit tests, and staged synthetic launch verification. |
| Socket protocol fidelity (CLI + extensions + hooks all depend on it) | Real protocol pinned in P2 (pipe/NUL, hook envelope); the untouched bash CLI as continuous acceptance test; replay captured 1.x traffic. |
| Installed CLI shim path coupling | Bundle ships `muxy-cli` at the legacy `Muxy_Muxy.bundle` path; verified in `verify-bundle.sh`. |
| 1.x Sparkle auto-update collides with 2.0 publishing | P15 sunset design (frozen final 1.x appcast, beta-tag cutover, Homebrew decision) BEFORE any 2.0 artifacts are published to the repo's release channels. |
| JSC C-API bridge (1.x used Obj-C JSContext — this is new work, not a port) | Spike first in P10: minimal host running a real extension's background script before building the platform. |
| WKWebView reparenting/focus across splits + data-store mapping | P9 vertical spike covers reparent + first-responder handoff + `…-00B0`→default-store mapping end-to-end. |
| libghostty fork-only APIs (`set_data_callback`, `read_cells`, `foreground_pid`, `send_input_raw`, `set_occlusion`) | Verify all five are exposed via ghostty-host before P8 scoping; fork exists if API changes are needed. |
| Idle freeing loses scrollback (no detach API — free+recreate) | Reproduce 1.x semantics exactly (scrollback loss documented; persistent-session-backed panes retain via replay). |
| Hook installer writes into other tools' configs | P11 requires current-mode permits at every mutator; development permits only explicit AI Notifications toggle/Refresh handlers; mechanically allowlist those mint sites; byte-compare staged shims/JSON against 1.x output. |
| Carbon hotkey + Input Monitoring from Rust | Small standalone spike in P6. |
| GPUI limitations (panel windows, transparency, multi-window quick terminal) | Spike quick-terminal window shape early in P6; fallback is an NSPanel via objc2 hosting a GPUI view. |
| Persistent-session protocol safety and lifecycle | Rust-only bounded codec tests, real PTY process tests, crash/restart continuity, same-UID socket isolation, complete OS-session reaping, and debug/release staged verification. |
| Scope: "full parity" is enormous | Stage gates; per-phase grill-me planning; parity checklist maintained from day one, seeded from the corrected inventory in this plan. |

## Verification strategy

1. **Per-phase:** unit tests in headless crates (pattern established, 303 tests); manual checklist per phase derived from exercising the Swift app; `scripts/check.sh` green locally through P14 and in CI from P15 onward.
2. **One-way migration harness:** synthetic Swift-shaped sources verify allowlists, destination-wins merge, source hashes, defaults filtering, source-missing completion, retry, abandonment, and terminal no-reinspection through staged debug and release launches.
3. **CLI acceptance:** installed 1.x `muxy` wrapper exercised against the dev socket from P2 onward, via the legacy bundle shim path.
4. **Linux guardrail:** per-package Linux `cargo check` (excluding ghostty-sys/host) + launch check on a Linux CI runner, introduced in P15 and required before release.
5. **Release gate (P16):** full parity checklist verified subsystem-by-subsystem; one-way migration acceptance green; CLI untouched and working; marketplace extensions load and run; signed and notarized multi-binary DMG; 1.x sunset executed per P15 design; beta-channel soak with you and beta users living on it; then promote to stable as 2.0.

## Out of scope for this roadmap

- Linux/Windows feature work beyond compile+launch guardrail.
- Sparkle (dropped; sunset handled in P15), Sentry (dropped; DSN injection removed, consent key preserved as unknown).
- New features beyond 1.x parity; redesigns beyond opportunistic polish.
- Deleting the stale legacy files found on real machines (preserved, ignored).
