# Architecture

Muxy is a Cargo workspace of 8 crates, layered so that domain logic, protocols, and services are platform-agnostic and testable, while GPUI and macOS-specific code live at the edges.

## Crate graph

```mermaid
flowchart TD
    muxy["muxy (binary)\nGPUI app, views, wiring"]
    api["muxy-api\nheadless services"]
    proto["muxy-proto\nportable wire codecs + framing"]
    core["muxy-core\ndomain model + persistence"]
    term["muxy-terminal\nterminal abstraction"]
    ui["muxy-ui\nreusable GPUI kit"]
    host["ghostty-host\nsafe libghostty wrapper"]
    sys["ghostty-sys\nbindgen FFI"]

    muxy --> api
    muxy --> proto
    muxy --> term
    muxy --> ui
    muxy --> core
    api --> core
    term --> core
    term -. macOS only .-> host
    muxy -. macOS only .-> host
    host --> sys
    sys --> lib["libghostty (C)"]
```

`muxy-proto` is a headless downstream dependency for socket callers. It owns portable codecs, framing, session policy, extension correlation, and the Unix listener. The `muxy` binary owns command names, permissions, app state, terminal resolution, and lifecycle wiring. The protocol crate never depends back on `muxy`, GPUI, domain stores, terminal crates, Ghostty, or Objective-C libraries.

## Responsibilities

| Crate | What lives there | Depends on GPUI? | macOS-only code? |
|---|---|---|---|
| `muxy` | app entry, `AppState`, all views, commands, keymap, terminal glue | yes | terminal backend |
| `muxy-api` | git, worktree lifecycle/hooks/locations, bounded subprocesses, project "truth", IDE detection, layouts, yaml, fs watcher, picker logic | no | Unix process-group edge only |
| `muxy-proto` | portable wire types, strict codecs, framing, and transport policy | no | Unix transport edge only |
| `muxy-core` | prefs, settings catalog, shortcuts, navigation history, stores, workspace/tab tree model | no | migration-only user-defaults access |
| `muxy-terminal` | backend trait, surface signals, search, scrollbar, confirmation | no | ghostty impl |
| `muxy-ui` | theme, icons, components, controls, text input, scrollbar | yes | SF Symbols |
| `ghostty-host` | runtime, surface, config, input, mouse — safe API over FFI | no | yes |
| `ghostty-sys` | raw bindgen bindings | no | yes |

## Socket stack

```mermaid
flowchart LR
    shim["installed muxy shim"] --> cli["untouched bundled muxy-cli"]
    cli --> unix["muxy-proto Unix server\nframing + sessions + codecs"]
    unix --> bridge["bounded typed request bridge"]
    bridge --> app["muxy socket runtime\ncatalog + permissions + dispatch"]
    app --> state2["AppState + persistent stores"]
    app --> surfaces["TerminalSurfaces"]
    app --> ingress["bounded hook / notification / extension sinks"]
    app -- "typed replies" --> unix
```

The socket runtime is owned by the app lifecycle and shuts down with the window/app. Debug and release socket names come from `muxy-core::environment`; the protocol crate receives an explicit path and contains no filename policy. The complete framing, routing, command ownership, and exclusion contract is documented in [Socket protocol](docs/development/socket-protocol.md).

## Terminal stack

```mermaid
flowchart LR
    subgraph muxy
        WV["window views"] --> TS["terminal::surfaces\nTerminalSurfaces"]
        TS --> BE["terminal::Backend\n(GhosttyBackend / Unsupported)"]
    end
    subgraph muxy-terminal
        HT["backend::TerminalSurfaceHandle\ntrait + signals"]
        GH["ghostty::host_view\nNSView embedding, scrollbar, IME"]
    end
    subgraph ghostty-host
        RT["runtime::GhosttyApp\nRuntimeEvent channel"]
        SF["surface::GhosttySurface"]
    end
    BE --> HT
    HT --> GH
    GH --> SF
    SF --> RT
    RT --> FFI["ghostty-sys → libghostty"]
    RT -- "RuntimeEvent (async-channel)" --> BE
```

Events flow back asynchronously: libghostty callbacks → `ghostty-host` `RuntimeEvent` channel → `muxy::terminal::TerminalEvents` → GPUI views.

## Workspace / tab model (muxy-core)

```mermaid
classDiagram
    class Workspace {
        projects: Vec~Project~
        groups: Groups
        active_group_id
    }
    class Project {
        id, name, path
    }
    class WorkspaceStore {
        states: Vec~WorkspaceState~
        raw snapshots (lossless JSON)
    }
    class WorkspaceState {
        per project / worktree
    }
    class TopLevelTabNode
    class Tab {
        kind: TabKind
    }
    class TabArea
    class SplitNode {
        Leaf | Split(Axis)
    }
    Workspace o-- Project
    Project o-- Worktree
    WorkspaceStore o-- WorkspaceState
    WorkspaceState o-- TopLevelTabNode
    TopLevelTabNode o-- Tab
    Tab o-- TabArea
    TabArea o-- SplitNode
```

## Persistence & external state

```mermaid
flowchart LR
    SWIFT["Retained Swift profile\nApp Support + NSUserDefaults"]
    MIG["One-way migration\nallowlist + destination wins"]
    subgraph release["Release: ~/.muxy"]
        RF["stores + ghostty.conf"]
        RP["preferences.json"]
        RS["swift-profile-migration.json"]
    end
    subgraph debug["Debug: ~/.muxy-dev"]
        DF["stores + ghostty.conf"]
        DP["preferences.json"]
    end
    PY["project .muxy/layouts/*.yml"]
    GIT["git CLI\n(worktrees, status)"]

    SWIFT --> MIG
    MIG --> release
    core["muxy-core\nprefs + stores\n(atomic writes)"] <--> release
    core <--> debug
    api["muxy-api"] --> PY
    api --> GIT
    api -- "watcher (notify)" --> release
    api -- "watcher (notify)" --> debug
    api -- "truth::refresh_truth" --> GIT
```

- Release reads the Swift profile only while its versioned migration state is pending. Completed, source-missing, and abandoned outcomes do not inspect it again.
- Normal preference reads and writes use private atomic `preferences.json`. `NSUserDefaults` is migration-only.
- File writes go through `store::persistence`; migration copies use unique destination-side temporary files and no-replace publication.
- `muxy-api::truth` recomputes per-project git facts off the UI thread; `watcher` retriggers on file changes.

## Worktree truth, lifecycle, and navigation

```mermaid
flowchart LR
    watch["watcher / startup / explicit refresh"] --> probe["muxy-api truth probe\nside-effect-free"]
    probe --> coord["app project coordinator\ngeneration + request identity"]
    coord -->|current| save["atomic worktree candidate save"]
    save --> apply["AppState apply"]
    ui["native UI or socket request"] --> op["serialized create / remove"]
    op --> life["muxy-api lifecycle\nlocation + hooks + Git + deadline"]
    life -->|disk outcome| apply
    apply --> ws["exact WorkspaceStore identity"]
    apply --> nav["NavigationHistory prune / record"]
    apply --> terms["terminal surface reconciliation"]
```

Truth probing never writes. `ProjectOperations` serializes explicit refresh, create, and remove per project, rejects
duplicates deterministically, and invalidates older generation/request identities before a candidate may be saved or
applied. Views render state and initiate requests; they do not own Git, hook, path, deadline, persistence, or navigation
policy.

`muxy-api` owns the disk-first lifecycle. Creation validates and resolves its location before Git, then never rolls back
a created checkout for later persistence or setup failures. Removal runs approved teardown and Git before exact
application cleanup. `AppState` removes workspace/raw-snapshot identity by project and worktree IDs, collects pane IDs
before deletion, prunes navigation, selects a surviving worktree in deterministic order, and retains that in-memory
result when post-disk persistence reports warnings.

`muxy-core::navigation` owns the 100-entry history, duplicate/forward truncation, live-target search, explicit cursor
commit, and pruning. App selection uses one navigation-aware workspace persistence seam; failed persistence restores the
previous selection and does not commit the cursor.

## Development and production policy

`muxy-core::environment` owns build-mode names and authorization facts.

```mermaid
flowchart LR
    C[Each consuming crate] --> M[build_mode!]
    M --> E[muxy-core::environment]
    E --> S[Socket names · P2]
    E --> D[Session paths · P8]
    E --> H[Hook paths and permits · P11]
    E --> P[Mobile keys and port · P12]
```

Each binary invokes `build_mode!` locally. No runtime environment variable selects development mode.

| State | Debug | Release | Staged test |
|---|---|---|---|
| Bundle identity | `com.muxy.dev` | `com.muxy.app` | `com.muxy.tests` |
| Root | `~/.muxy-dev` | `~/.muxy` | Injected ignored root |
| Preferences | Root-local `preferences.json` | Root-local `preferences.json` | Root-local `preferences.json` |
| Ghostty configuration | Root-local | Root-local | Root-local |
| Swift import | Never | One bounded migration | Injected synthetic source only |
| Runtime names | Development policy | Production policy | Selected build policy |

Debug and release select their roots by compile-time build mode. Environment overrides are honored only by recognized test executables. Mobile runtime implementation remains assigned to P12.

## App wiring (muxy binary)

```mermaid
flowchart TD
    main["main.rs\nGPUI app boot"] --> state["AppState\nprefs, workspace, worktrees,\nnavigation, coordinator"]
    main --> win["views::window\nrender, menus, orchestration"]
    win --> wsv["workspace_view + sidebar\n+ titlebar + status bar"]
    win --> term2["window::terminal\nsurface hosting"]
    win --> ov["overlays: omnibox, project picker,\nsettings, shortcut editor, switcher"]
    cmd["command.rs + keymap.rs"] --> win
    ov -- "picker logic" --> papi["muxy-api::picker\n(navigator, search, session)"]
    wsv -- reads/mutates --> state
    state -- persists via --> mcore["muxy-core stores"]
```

Views own no business logic: pickers, search, Git truth, worktree lifecycle, bounded processes, and layout parsing are called out to `muxy-api`; navigation and tab-tree mutations go through `muxy-core`; rendering primitives come from `muxy-ui`.

## Platform strategy

```mermaid
flowchart LR
    common["muxy-core + muxy-api + muxy-proto + muxy-ui\n+ muxy-terminal traits"] --> mac["macOS: ghostty backend\n(ghostty-host/sys)"]
    common --> other["Linux / Windows:\nUnsupportedBackend (stub today)"]
```

Everything above the `muxy-terminal::backend` trait is platform-neutral. Worktree, navigation, lifecycle, hook, Git, persistence, and coordinator policy is shared Rust. Unix subprocess cleanup uses a process group on macOS and Linux; other targets use a direct-child fallback until they gain an equivalent descendant-kill edge. Path reveal, native file dialogs, the current Unix socket host, and terminal embedding remain platform edges. Porting means implementing a backend and those small `cfg(target_os)` islands without moving policy into them.
