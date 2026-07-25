# AI Session Restore — Design

Date: 2026-07-25
Status: Approved for planning

## Problem

Muxy does not persist AI chat history. When a workspace is restored, agent panes
reopen as bare shells and the live `claude`/`codex`/etc. conversation is gone.

Built-in restore existed once (PR #422), was disabled by default (#518), then
dropped from core and handed to extensions (#725). A follow-up that resumed Claude
sessions (#727) was rejected. So today, restore only rebuilds layout + working
directory; it captures no command, scrollback, or conversation.

## Goal

Bring restore back, improved, with seamless DX: when a workspace is restored, each
agent pane **reattaches its real prior conversation** by resuming the AI tool's own
session — not by re-running a command or replaying captured scrollback.

## Decisions (from brainstorming)

- **Placement: Hybrid.** Core captures and exposes agent/session facts; a thin
  bundled extension owns restore policy and execution. Preserves the #725 boundary
  (core never runs a restore command itself) and gives one clean kill switch.
- **Default restore UX: Auto-resume immediately**, gated behind a single global
  opt-in (default OFF). Opting in enables the bundled extension.
- **Agent scope: all providers that support resume**, via a pluggable per-provider
  strategy. v1 targets the user's five: Claude, Codex, Cursor, agy (Antigravity),
  Hermes. Providers that declare no strategy degrade to bare-shell restore.
- **Session identity: capture exact id, fall back to continue-latest.** Layered by
  reliability: hook-reported id → Muxy-injected id → store-discovered id →
  provider continue-latest verb → plain shell.

## Non-goals (deferred / YAGNI)

- Capturing and replaying terminal scrollback text. The AI tool owns the history;
  we reattach its session instead.
- Cross-machine session sync, an in-app unified chat viewer, per-session
  naming/pinning UI. Each is its own project.

## Grounded research — the five agents' session stores

Verified on disk. Every one of the user's agents is directory-scopable through one
of three access patterns.

| Agent | Store | Dir-keyed | Access pattern | Resume id | Preview data |
|-------|-------|-----------|----------------|-----------|--------------|
| Claude | `~/.claude/projects/<cwd:/→->/<uuid>.jsonl` | by folder | glob one folder (O(1)) | filename `<uuid>` | `cwd`, `gitBranch`, `timestamp`, `version`, first user msg |
| Cursor | `~/.cursor/projects/<cwd:/→->/agent-transcripts/<chatid>/<chatid>.jsonl` | by folder | glob one folder (O(1)) | dir `<chat-uuid>` | JSONL `<user_query>` first line |
| agy (Antigravity) | `~/.gemini/antigravity-cli/` | by index | `cache/last_conversations.json` maps `cwd → conversation-uuid`; full log in `conversations/<uuid>.db` | conversation `<uuid>` | title/summary in `.db`, `agyhub_summaries` |
| Hermes | `~/.hermes/state.db` (SQLite) | by column | `SELECT … WHERE cwd=?` on `sessions` table | `sessions.id` | `title`, `git_branch`, `git_repo_root`, `started_at`, `pinned`, `archived` |
| Codex | `~/.codex/sessions/YYYY/MM/DD/rollout-<ts>-<uuid>.jsonl` | not by folder | scan each rollout line-1 `session_meta.payload.cwd`; cache by mtime (`session_index.jsonl` has no cwd) | header `session_id` | `cwd`, `cli_version`, timestamp |

Notes:
- agy's `last_conversations.json` only holds the *latest* conversation per dir;
  older sessions require enumerating `conversations/*.db`.
- Hermes per-session JSON files (`~/.hermes/sessions/*.json`) carry no cwd — the
  `state.db` `sessions` table is the source of truth.
- Resume verbs confirmed for Claude (`--resume <id>` / `--continue`) and Codex
  (`resume <id>` / `resume --last`). Cursor / agy / Hermes: store + id are certain;
  the exact resume flag is verified at implementation, not guessed.

### Three store adapters

```
FolderGlobStore   → Claude, Cursor   (glob <slug>/, read 1 line each for preview)
IndexStore        → agy, Hermes       (read a map file / SQL query — O(1))
RolloutScanStore  → Codex             (scan + cache rollout headers by cwd)
```

**Guardrail:** these are undocumented private formats that will drift. They power the
session picker and the discovery fallback only — never the primary capture path. Each
adapter is isolated and independently testable against a fixture store, so a format
change breaks one adapter loudly, not the feature.

## Architecture — core / extension split

Core captures and exposes facts; the extension decides policy and executes.

**Core (Muxy app) adds:**
- `AgentSessionRegistry` — `[paneID: AgentSession]`, where
  `AgentSession = { providerID, sessionID?, cwd, startedAt, source }` and
  `source ∈ { .launched, .detected, .hookReported }`. Fed by three existing feeds:
  the hook bridge (session_id), `DetectedAgentStore` (which provider),
  Muxy-initiated launches (injected id).
- `TerminalTabSnapshot.agentSession: AgentSession?` — additive. Existing snapshots
  decode unchanged (all fields are `decodeIfPresent`). Stores only
  `{providerID, sessionID?, cwd}`; no scrollback.
- Provider `AgentResumeStrategy` + optional `AgentSessionStore` — the pluggable seam.
- One new extension event `agent.session` (paneID, providerID, sessionID, cwd).
- One new read-only API verb `agent.resolveResume(providerID, sessionID?, cwd)` →
  returns the resolved resume command (or null). This is the *only* new API surface;
  it runs the provider seam's degradation ladder on demand, so staleness is handled
  at restore time rather than baked into a stored command.
- `tabs.open` already accepts a consent-gated `command`, which is the restore
  primitive — no new restore-execution API needed.

**Extension (bundled `session-restore`, replaces `demo-session-restore`):**
- Subscribes to `tab.*` + `agent.session`, persists a restore manifest of
  `{tab, providerID, sessionID?, cwd}`.
- On restore, calls `agent.resolveResume` to get a fresh command, then runs it via
  consent-gated `tabs.open(dir, command)`. Auto-fires because the user opted in
  globally. If resolve returns null, it opens a plain shell in `cwd`.

## Data flow

**Capture (live).** A pane gets an `AgentSession` from whichever feed fires first:
- Hook-reported (primary): `SessionStart` hook POSTs `session_id` + `cwd` through the
  existing MuxyHookBridge socket → registry, `source:.hookReported`. No file parsing.
- Muxy-launched: when Muxy controls argv, it stamps the id it passed,
  `source:.launched`.
- Detected (fallback): `ForegroundProcessInspector` + `AIAgentDetector` identify the
  provider even for user-typed `claude`; record `{providerID, cwd, sessionID:nil,
  source:.detected}`, resolved later by the store adapter.

The registry emits `agent.session` on every change.

**Snapshot (debounced + on quit).** `WorkspaceRestorer.snapshotAll` reads the registry
and writes optional `agentSession` onto each `TerminalTabSnapshot`. Layout persistence
unchanged; additive; old snapshots still decode.

**Restore (at load).** For each restored tab with an `agentSession`:
1. The extension calls `agent.resolveResume(providerID, sessionID?, cwd)`. Core runs
   the provider's `resumeStrategy`: `sessionID` present → exact; else
   `AgentSessionStore.sessions(inDirectory: cwd)` newest → exact; else provider
   continue-latest verb; else null.
2. The extension runs the returned command via consent-gated `tabs.open(dir,
   command)`, or opens a plain shell if null. Core resolves; the extension executes —
   preserving the #725 boundary.

## Provider seam

```swift
protocol AgentResumeStrategy {
    func resumeCommand(sessionID: String?, cwd: String) -> String?
    var continueLatestCommand: String? { get }
}

protocol AgentSessionStore {
    func sessions(inDirectory: String) -> [AgentSessionRef]
}

struct AgentSessionRef {
    let id: String
    let providerID: String
    let cwd: String
    let gitBranch: String?
    let title: String?
    let preview: String?
    let updatedAt: Date
    let pinned: Bool
    let archived: Bool
}
```

v1 wiring (⚠️ = resume flag verified at implementation, not guessed):

| Provider | AgentSessionStore | resume by id | continue-latest |
|----------|-------------------|-------------|-----------------|
| Claude | FolderGlob | `claude --resume <id>` | `claude --continue` |
| Codex | RolloutScan (cached) | `codex resume <id>` | `codex resume --last` |
| Cursor | FolderGlob | `cursor-agent --resume=<id>` ⚠️ | ⚠️ |
| agy | IndexMap (`last_conversations.json`) | ⚠️ | ⚠️ |
| Hermes | SQLite (`state.db`) | ⚠️ | ⚠️ |

## Consent, safety & error handling

- **Global opt-in** (default OFF): Settings toggle "Auto-resume AI sessions on
  restore". This is the enable switch for the bundled extension. Off ⇒ today's
  behavior exactly. Directly answers why #518/#725 pulled back — restore never runs
  a command unless the user turned this on.
- **Command allow-listing:** the extension only runs a command core resolved from a
  known provider's resume strategy (fixed verb + id/dir), never arbitrary captured
  shell text. The old command blacklist is obsolete.
- **Degradation ladder:** exact id → store-discovered id → continue-latest → plain
  shell. Any failure at any rung falls to the next. Worst case is a normal shell,
  never an error dialog.
- **Store drift isolation:** an adapter parse failure logs and returns `[]`, dropping
  that provider to continue-latest.
- **Staleness:** a resolved session whose store entry no longer exists falls through
  to continue-latest / plain shell. No dangling `--resume <dead-id>`.

## Testing

- **Unit (core):** `AgentSessionRegistry` feed precedence; `TerminalTabSnapshot`
  additive round-trip (old snapshot without `agentSession` decodes); each
  `resumeStrategy` command string; degradation ladder picks the right rung.
- **Adapter (fixtures):** a committed fixture store per agent (fake
  `~/.claude/projects/…`, a Codex rollout, agy `last_conversations.json`, a Hermes
  `state.db`) → assert `sessions(inDirectory:)` returns expected refs + previews.
  Catches format drift loudly.
- **Extension:** a `demo-`-prefixed / bundled extension exercising `agent.session`
  → `tabs.open`; SKILL + docs updated per CLAUDE.md extension rules.
- **E2E (manual, user runs):** opt in, run `claude` in a project, quit, relaunch →
  pane returns mid-conversation. Repeat for Codex.

## Documentation impact (CLAUDE.md rules)

- Update the muxy-extension SKILL + docs for the new `agent.session` event and the
  `tabs.open` command-consent restore flow.
- Offer a `demo-session-restore` extension at `~/.config/muxy/extensions`.
- No inline code comments (self-explanatory code).
