# AI Session Restore Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** On workspace restore, each agent pane reattaches its real prior AI conversation by resuming the AI tool's own session, gated behind a global opt-in.

**Architecture:** Hybrid. Core adds a pluggable per-provider resume seam (`AgentResumeStrategy` + `AgentSessionStore`), records the running agent's session per pane (`AgentSessionRegistry`), persists it additively on `TerminalTabSnapshot`, and exposes one read-only API verb `agent.resolveResume`. A thin bundled extension listens for a new `agent.session` event, persists a manifest, and on restore calls `agent.resolveResume` then `tabs.open(dir, command)`. Core never runs the restore command itself — the extension does, preserving the #725 boundary.

**Tech Stack:** Swift 6, SPM, swift-testing (`@Suite`/`@Test`/`#expect`), macOS 14+. No new dependencies. SQLite reads for Hermes/agy use the system `libsqlite3` already linkable on macOS, or shell out to `sqlite3` — see Task 6/5.

## Global Constraints

- No code comments anywhere. Code must be self-explanatory.
- Early returns over nested conditionals.
- Swift 6.0+, macOS 14+. Tests use `swift-testing`, isolated Application Support storage.
- Run `scripts/checks.sh --fix` after every task; it must pass (format, lint `--strict`, build, test).
- Provider resume/session stores read undocumented third-party formats: every adapter must fail soft (log + return `[]`/`nil`), never throw into restore.
- Store adapters are best-effort discovery only; they are never the sole capture path.
- Extension/API changes require updating the muxy-extension SKILL + docs (Task 12).
- Resume CLI flags for Cursor/agy/Hermes are unverified (⚠). Each relevant task has a verification step to confirm the exact flag from the installed CLI before hardcoding it.

---

### Task 1: Provider resume seam — protocols and value types

**Files:**
- Create: `Muxy/Services/AI/Resume/AgentSessionRef.swift`
- Create: `Muxy/Services/AI/Resume/AgentResumeProviding.swift`
- Test: `Tests/MuxyTests/Services/AI/Resume/AgentResumeProvidingTests.swift`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `struct AgentSessionRef` with `let id: String; let providerID: String; let cwd: String; let gitBranch: String?; let title: String?; let preview: String?; let updatedAt: Date; let pinned: Bool; let archived: Bool` (memberwise init, `Equatable`).
  - `protocol AgentResumeStrategy { func resumeCommand(sessionID: String?, cwd: String) -> String?; var continueLatestCommand: String? { get } }`
  - `protocol AgentSessionStore { func sessions(inDirectory directory: String) -> [AgentSessionRef] }`
  - `protocol AgentResumeProviding { var resumeStrategy: AgentResumeStrategy? { get }; var sessionStore: AgentSessionStore? { get } }`
  - Default extension: `extension AgentResumeProviding { var resumeStrategy: AgentResumeStrategy? { nil }; var sessionStore: AgentSessionStore? { nil } }`

- [ ] **Step 1: Write the failing test**

```swift
import Foundation
import Testing

@testable import Muxy

@Suite("AgentResumeProviding defaults")
struct AgentResumeProvidingTests {
    private struct BareProvider: AgentResumeProviding {}

    @Test("a provider with no strategy exposes nil seams")
    func defaultsAreNil() {
        let provider = BareProvider()
        #expect(provider.resumeStrategy == nil)
        #expect(provider.sessionStore == nil)
    }

    @Test("AgentSessionRef is value-equal")
    func sessionRefEquatable() {
        let date = Date(timeIntervalSince1970: 0)
        let a = AgentSessionRef(id: "1", providerID: "claude", cwd: "/x", gitBranch: nil,
                                title: nil, preview: nil, updatedAt: date, pinned: false, archived: false)
        let b = AgentSessionRef(id: "1", providerID: "claude", cwd: "/x", gitBranch: nil,
                                title: nil, preview: nil, updatedAt: date, pinned: false, archived: false)
        #expect(a == b)
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `swift test --filter AgentResumeProvidingTests 2>&1 | tail -20`
Expected: FAIL — `cannot find 'AgentResumeProviding' in scope`.

- [ ] **Step 3: Write minimal implementation**

`Muxy/Services/AI/Resume/AgentSessionRef.swift`:

```swift
import Foundation

struct AgentSessionRef: Equatable {
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

`Muxy/Services/AI/Resume/AgentResumeProviding.swift`:

```swift
import Foundation

protocol AgentResumeStrategy {
    func resumeCommand(sessionID: String?, cwd: String) -> String?
    var continueLatestCommand: String? { get }
}

protocol AgentSessionStore {
    func sessions(inDirectory directory: String) -> [AgentSessionRef]
}

protocol AgentResumeProviding {
    var resumeStrategy: AgentResumeStrategy? { get }
    var sessionStore: AgentSessionStore? { get }
}

extension AgentResumeProviding {
    var resumeStrategy: AgentResumeStrategy? { nil }
    var sessionStore: AgentSessionStore? { nil }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `swift test --filter AgentResumeProvidingTests 2>&1 | tail -20`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add Muxy/Services/AI/Resume Tests/MuxyTests/Services/AI/Resume
git commit -m "feat: add agent resume seam protocols and session ref"
```

---

### Task 2: Claude adapter — folder-glob store + resume strategy

**Files:**
- Create: `Muxy/Services/AI/Resume/ClaudeSessionStore.swift`
- Create: `Muxy/Services/AI/Resume/ClaudeResumeStrategy.swift`
- Modify: `Muxy/Services/Providers/ClaudeCodeProvider.swift` (add `AgentResumeProviding` conformance)
- Test: `Tests/MuxyTests/Services/AI/Resume/ClaudeSessionStoreTests.swift`

**Interfaces:**
- Consumes: `AgentSessionRef`, `AgentResumeStrategy`, `AgentSessionStore` (Task 1).
- Produces:
  - `struct ClaudeSessionStore: AgentSessionStore` with `init(homeDirectory: String = NSHomeDirectory())`.
  - `struct ClaudeResumeStrategy: AgentResumeStrategy`.
  - Store rule: directory `/a/b/c` → folder `~/.claude/projects/-a-b-c/`; each `<uuid>.jsonl` is one session; `id` = filename without extension; `updatedAt` = file mtime; `preview` = first line whose decoded JSON has `message.role == "user"`, truncated to 120 chars.

- [ ] **Step 1: Write the failing test**

```swift
import Foundation
import Testing

@testable import Muxy

@Suite("ClaudeSessionStore")
struct ClaudeSessionStoreTests {
    private func makeHome() throws -> String {
        let home = NSTemporaryDirectory() + "claude-store-" + UUID().uuidString
        try FileManager.default.createDirectory(atPath: home, withIntermediateDirectories: true)
        return home
    }

    @Test("lists sessions for a directory by slugging the path")
    func listsSessions() throws {
        let home = try makeHome()
        let dir = "/Users/x/proj"
        let slugDir = home + "/.claude/projects/-Users-x-proj"
        try FileManager.default.createDirectory(atPath: slugDir, withIntermediateDirectories: true)
        let line = #"{"type":"user","message":{"role":"user","content":"hello world"},"sessionId":"abc"}"#
        try (line + "\n").write(toFile: slugDir + "/abc.jsonl", atomically: true, encoding: .utf8)

        let store = ClaudeSessionStore(homeDirectory: home)
        let sessions = store.sessions(inDirectory: dir)

        #expect(sessions.count == 1)
        #expect(sessions.first?.id == "abc")
        #expect(sessions.first?.providerID == "claude")
        #expect(sessions.first?.preview?.contains("hello world") == true)
    }

    @Test("returns empty for a directory with no store folder")
    func emptyWhenMissing() throws {
        let store = ClaudeSessionStore(homeDirectory: try makeHome())
        #expect(store.sessions(inDirectory: "/nope").isEmpty)
    }

    @Test("resume strategy prefers explicit id and falls back to continue")
    func resumeStrategy() {
        let strategy = ClaudeResumeStrategy()
        #expect(strategy.resumeCommand(sessionID: "abc", cwd: "/x") == "claude --resume abc")
        #expect(strategy.resumeCommand(sessionID: nil, cwd: "/x") == "claude --continue")
        #expect(strategy.continueLatestCommand == "claude --continue")
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `swift test --filter ClaudeSessionStoreTests 2>&1 | tail -20`
Expected: FAIL — `cannot find 'ClaudeSessionStore' in scope`.

- [ ] **Step 3: Write minimal implementation**

`Muxy/Services/AI/Resume/ClaudeResumeStrategy.swift`:

```swift
import Foundation

struct ClaudeResumeStrategy: AgentResumeStrategy {
    func resumeCommand(sessionID: String?, cwd _: String) -> String? {
        guard let sessionID, !sessionID.isEmpty else { return continueLatestCommand }
        return "claude --resume \(sessionID)"
    }

    var continueLatestCommand: String? { "claude --continue" }
}
```

`Muxy/Services/AI/Resume/ClaudeSessionStore.swift`:

```swift
import Foundation
import os

struct ClaudeSessionStore: AgentSessionStore {
    private static let logger = Logger(subsystem: "app.muxy", category: "ClaudeSessionStore")
    private let homeDirectory: String

    init(homeDirectory: String = NSHomeDirectory()) {
        self.homeDirectory = homeDirectory
    }

    func sessions(inDirectory directory: String) -> [AgentSessionRef] {
        let slug = directory.replacingOccurrences(of: "/", with: "-")
        let folder = homeDirectory + "/.claude/projects/" + slug
        let fileManager = FileManager.default
        guard let entries = try? fileManager.contentsOfDirectory(atPath: folder) else { return [] }

        return entries.compactMap { entry in
            guard entry.hasSuffix(".jsonl") else { return nil }
            let path = folder + "/" + entry
            let id = String(entry.dropLast(".jsonl".count))
            let attributes = try? fileManager.attributesOfItem(atPath: path)
            let updatedAt = (attributes?[.modificationDate] as? Date) ?? Date(timeIntervalSince1970: 0)
            return AgentSessionRef(
                id: id,
                providerID: "claude",
                cwd: directory,
                gitBranch: nil,
                title: nil,
                preview: Self.firstUserMessage(atPath: path),
                updatedAt: updatedAt,
                pinned: false,
                archived: false
            )
        }
        .sorted { $0.updatedAt > $1.updatedAt }
    }

    private static func firstUserMessage(atPath path: String) -> String? {
        guard let contents = try? String(contentsOfFile: path, encoding: .utf8) else { return nil }
        for line in contents.split(separator: "\n") {
            guard let data = line.data(using: .utf8),
                  let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let message = object["message"] as? [String: Any],
                  (message["role"] as? String) == "user"
            else { continue }
            let text = Self.plainText(from: message["content"]) ?? ""
            return String(text.prefix(120))
        }
        return nil
    }

    private static func plainText(from content: Any?) -> String? {
        if let string = content as? String { return string }
        if let blocks = content as? [[String: Any]] {
            return blocks.compactMap { $0["text"] as? String }.joined(separator: " ")
        }
        return nil
    }
}
```

In `Muxy/Services/Providers/ClaudeCodeProvider.swift`, add conformance to the type declaration and the two computed seams:

```swift
struct ClaudeCodeProvider: AIProviderIntegration, AIAgentLaunchProvider, AgentResumeProviding {
    var resumeStrategy: AgentResumeStrategy? { ClaudeResumeStrategy() }
    var sessionStore: AgentSessionStore? { ClaudeSessionStore() }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `swift test --filter ClaudeSessionStoreTests 2>&1 | tail -20`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add Muxy/Services/AI/Resume Muxy/Services/Providers/ClaudeCodeProvider.swift Tests/MuxyTests/Services/AI/Resume/ClaudeSessionStoreTests.swift
git commit -m "feat: add Claude session store and resume strategy"
```

---

### Task 3: Codex adapter — rollout-scan store + resume strategy

**Files:**
- Create: `Muxy/Services/AI/Resume/CodexSessionStore.swift`
- Create: `Muxy/Services/AI/Resume/CodexResumeStrategy.swift`
- Modify: `Muxy/Services/Providers/CodexProvider.swift` (add `AgentResumeProviding` conformance)
- Test: `Tests/MuxyTests/Services/AI/Resume/CodexSessionStoreTests.swift`

**Interfaces:**
- Consumes: `AgentSessionRef`, seam protocols (Task 1).
- Produces:
  - `struct CodexSessionStore: AgentSessionStore` with `init(homeDirectory: String = NSHomeDirectory())`.
  - `struct CodexResumeStrategy: AgentResumeStrategy`.
  - Store rule: recursively scan `~/.codex/sessions/**/rollout-*.jsonl`; read only line 1 (`{"type":"session_meta","payload":{"session_id":…,"cwd":…}}`); keep files whose `payload.cwd == directory`; `id` = `payload.session_id`; `updatedAt` = file mtime.

- [ ] **Step 1: Write the failing test**

```swift
import Foundation
import Testing

@testable import Muxy

@Suite("CodexSessionStore")
struct CodexSessionStoreTests {
    private func makeHome() throws -> String {
        let home = NSTemporaryDirectory() + "codex-store-" + UUID().uuidString
        try FileManager.default.createDirectory(atPath: home + "/.codex/sessions/2026/07/03",
                                                withIntermediateDirectories: true)
        return home
    }

    @Test("matches rollout files by cwd in the session_meta header")
    func matchesByCwd() throws {
        let home = try makeHome()
        let target = "/Users/x/proj"
        let dir = home + "/.codex/sessions/2026/07/03"
        let header = #"{"type":"session_meta","payload":{"session_id":"sid-1","cwd":"/Users/x/proj"}}"#
        try (header + "\n{\"type\":\"event\"}\n").write(
            toFile: dir + "/rollout-2026-07-03T04-32-05-sid-1.jsonl", atomically: true, encoding: .utf8)
        let other = #"{"type":"session_meta","payload":{"session_id":"sid-2","cwd":"/other"}}"#
        try (other + "\n").write(
            toFile: dir + "/rollout-2026-07-03T04-40-00-sid-2.jsonl", atomically: true, encoding: .utf8)

        let store = CodexSessionStore(homeDirectory: home)
        let sessions = store.sessions(inDirectory: target)

        #expect(sessions.count == 1)
        #expect(sessions.first?.id == "sid-1")
        #expect(sessions.first?.providerID == "codex")
    }

    @Test("resume strategy uses resume <id> and resume --last")
    func resumeStrategy() {
        let strategy = CodexResumeStrategy()
        #expect(strategy.resumeCommand(sessionID: "sid-1", cwd: "/x") == "codex resume sid-1")
        #expect(strategy.resumeCommand(sessionID: nil, cwd: "/x") == "codex resume --last")
        #expect(strategy.continueLatestCommand == "codex resume --last")
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `swift test --filter CodexSessionStoreTests 2>&1 | tail -20`
Expected: FAIL — `cannot find 'CodexSessionStore' in scope`.

- [ ] **Step 3: Write minimal implementation**

`Muxy/Services/AI/Resume/CodexResumeStrategy.swift`:

```swift
import Foundation

struct CodexResumeStrategy: AgentResumeStrategy {
    func resumeCommand(sessionID: String?, cwd _: String) -> String? {
        guard let sessionID, !sessionID.isEmpty else { return continueLatestCommand }
        return "codex resume \(sessionID)"
    }

    var continueLatestCommand: String? { "codex resume --last" }
}
```

`Muxy/Services/AI/Resume/CodexSessionStore.swift`:

```swift
import Foundation

struct CodexSessionStore: AgentSessionStore {
    private let homeDirectory: String

    init(homeDirectory: String = NSHomeDirectory()) {
        self.homeDirectory = homeDirectory
    }

    func sessions(inDirectory directory: String) -> [AgentSessionRef] {
        let root = homeDirectory + "/.codex/sessions"
        let fileManager = FileManager.default
        guard let enumerator = fileManager.enumerator(atPath: root) else { return [] }

        var results: [AgentSessionRef] = []
        for case let relative as String in enumerator {
            guard relative.hasSuffix(".jsonl"),
                  (relative as NSString).lastPathComponent.hasPrefix("rollout-")
            else { continue }
            let path = root + "/" + relative
            guard let header = Self.header(atPath: path),
                  let payload = header["payload"] as? [String: Any],
                  (payload["cwd"] as? String) == directory,
                  let sessionID = payload["session_id"] as? String
            else { continue }
            let attributes = try? fileManager.attributesOfItem(atPath: path)
            let updatedAt = (attributes?[.modificationDate] as? Date) ?? Date(timeIntervalSince1970: 0)
            results.append(AgentSessionRef(
                id: sessionID, providerID: "codex", cwd: directory, gitBranch: nil,
                title: nil, preview: nil, updatedAt: updatedAt, pinned: false, archived: false))
        }
        return results.sorted { $0.updatedAt > $1.updatedAt }
    }

    private static func header(atPath path: String) -> [String: Any]? {
        guard let handle = FileManager.default.contents(atPath: path),
              let text = String(data: handle, encoding: .utf8),
              let firstLine = text.split(separator: "\n", maxSplits: 1).first,
              let data = firstLine.data(using: .utf8)
        else { return nil }
        return try? JSONSerialization.jsonObject(with: data) as? [String: Any]
    }
}
```

In `Muxy/Services/Providers/CodexProvider.swift`:

```swift
struct CodexProvider: AIProviderIntegration, AgentResumeProviding {
    var resumeStrategy: AgentResumeStrategy? { CodexResumeStrategy() }
    var sessionStore: AgentSessionStore? { CodexSessionStore() }
```

(Keep any existing conformances Codex already declares; append `AgentResumeProviding`.)

- [ ] **Step 4: Run test to verify it passes**

Run: `swift test --filter CodexSessionStoreTests 2>&1 | tail -20`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add Muxy/Services/AI/Resume Muxy/Services/Providers/CodexProvider.swift Tests/MuxyTests/Services/AI/Resume/CodexSessionStoreTests.swift
git commit -m "feat: add Codex session store and resume strategy"
```

---

### Task 4: Cursor adapter — folder-glob store + resume strategy

**Files:**
- Create: `Muxy/Services/AI/Resume/CursorSessionStore.swift`
- Create: `Muxy/Services/AI/Resume/CursorResumeStrategy.swift`
- Modify: `Muxy/Services/Providers/CursorProvider.swift`
- Test: `Tests/MuxyTests/Services/AI/Resume/CursorSessionStoreTests.swift`

**Interfaces:**
- Consumes: seam protocols (Task 1).
- Produces:
  - `struct CursorSessionStore: AgentSessionStore` — directory `/a/b/c` → folder `~/.cursor/projects/a-b-c/agent-transcripts/` (leading `/` dropped, remaining `/`→`-`); each subdirectory `<chatid>/<chatid>.jsonl`; `id` = `<chatid>`; `preview` = first line's `message.content[0].text` (strips `<user_query>` tags), 120 chars; `updatedAt` = mtime of the `.jsonl`.
  - `struct CursorResumeStrategy: AgentResumeStrategy`.

- [ ] **Step 0: Verify the resume flag (⚠)**

Run: `cursor-agent --help 2>&1 | grep -iE "resume|--session|continue"`
Confirm the exact resume flag. If it differs from `cursor-agent --resume=<id>`, update the strings in Step 3 and the test in Step 1 accordingly. If `cursor-agent` exposes no resume flag, set both `resumeCommand` (with id) and `continueLatestCommand` to `nil` and adjust the test to expect `nil`.

- [ ] **Step 1: Write the failing test**

```swift
import Foundation
import Testing

@testable import Muxy

@Suite("CursorSessionStore")
struct CursorSessionStoreTests {
    @Test("lists chats under the dir-keyed transcripts folder")
    func listsChats() throws {
        let home = NSTemporaryDirectory() + "cursor-store-" + UUID().uuidString
        let chat = home + "/.cursor/projects/Users-x-proj/agent-transcripts/chat-1"
        try FileManager.default.createDirectory(atPath: chat, withIntermediateDirectories: true)
        let line = #"{"role":"user","message":{"content":[{"type":"text","text":"<user_query>\nDeploy\n</user_query>"}]}}"#
        try (line + "\n").write(toFile: chat + "/chat-1.jsonl", atomically: true, encoding: .utf8)

        let store = CursorSessionStore(homeDirectory: home)
        let sessions = store.sessions(inDirectory: "/Users/x/proj")

        #expect(sessions.count == 1)
        #expect(sessions.first?.id == "chat-1")
        #expect(sessions.first?.preview?.contains("Deploy") == true)
        #expect(sessions.first?.preview?.contains("<user_query>") == false)
    }

    @Test("resume strategy builds the resume command")
    func resumeStrategy() {
        let strategy = CursorResumeStrategy()
        #expect(strategy.resumeCommand(sessionID: "chat-1", cwd: "/x") == "cursor-agent --resume=chat-1")
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `swift test --filter CursorSessionStoreTests 2>&1 | tail -20`
Expected: FAIL — `cannot find 'CursorSessionStore' in scope`.

- [ ] **Step 3: Write minimal implementation**

`Muxy/Services/AI/Resume/CursorResumeStrategy.swift`:

```swift
import Foundation

struct CursorResumeStrategy: AgentResumeStrategy {
    func resumeCommand(sessionID: String?, cwd _: String) -> String? {
        guard let sessionID, !sessionID.isEmpty else { return continueLatestCommand }
        return "cursor-agent --resume=\(sessionID)"
    }

    var continueLatestCommand: String? { nil }
}
```

`Muxy/Services/AI/Resume/CursorSessionStore.swift`:

```swift
import Foundation

struct CursorSessionStore: AgentSessionStore {
    private let homeDirectory: String

    init(homeDirectory: String = NSHomeDirectory()) {
        self.homeDirectory = homeDirectory
    }

    func sessions(inDirectory directory: String) -> [AgentSessionRef] {
        let trimmed = directory.hasPrefix("/") ? String(directory.dropFirst()) : directory
        let slug = trimmed.replacingOccurrences(of: "/", with: "-")
        let folder = homeDirectory + "/.cursor/projects/" + slug + "/agent-transcripts"
        let fileManager = FileManager.default
        guard let chats = try? fileManager.contentsOfDirectory(atPath: folder) else { return [] }

        return chats.compactMap { chatID in
            let path = folder + "/" + chatID + "/" + chatID + ".jsonl"
            guard fileManager.fileExists(atPath: path) else { return nil }
            let attributes = try? fileManager.attributesOfItem(atPath: path)
            let updatedAt = (attributes?[.modificationDate] as? Date) ?? Date(timeIntervalSince1970: 0)
            return AgentSessionRef(
                id: chatID, providerID: "cursor", cwd: directory, gitBranch: nil,
                title: nil, preview: Self.firstUserText(atPath: path),
                updatedAt: updatedAt, pinned: false, archived: false)
        }
        .sorted { $0.updatedAt > $1.updatedAt }
    }

    private static func firstUserText(atPath path: String) -> String? {
        guard let contents = try? String(contentsOfFile: path, encoding: .utf8),
              let firstLine = contents.split(separator: "\n").first,
              let data = firstLine.data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let message = object["message"] as? [String: Any],
              let blocks = message["content"] as? [[String: Any]]
        else { return nil }
        let text = blocks.compactMap { $0["text"] as? String }.joined(separator: " ")
        let cleaned = text
            .replacingOccurrences(of: "<user_query>", with: "")
            .replacingOccurrences(of: "</user_query>", with: "")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return String(cleaned.prefix(120))
    }
}
```

In `Muxy/Services/Providers/CursorProvider.swift`, append `AgentResumeProviding` and add:

```swift
    var resumeStrategy: AgentResumeStrategy? { CursorResumeStrategy() }
    var sessionStore: AgentSessionStore? { CursorSessionStore() }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `swift test --filter CursorSessionStoreTests 2>&1 | tail -20`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add Muxy/Services/AI/Resume Muxy/Services/Providers/CursorProvider.swift Tests/MuxyTests/Services/AI/Resume/CursorSessionStoreTests.swift
git commit -m "feat: add Cursor session store and resume strategy"
```

---

### Task 5: agy (Antigravity) adapter — index-map store + resume strategy

**Files:**
- Create: `Muxy/Services/AI/Resume/AgyProvider.swift` (new provider — agy is not in the existing registry)
- Create: `Muxy/Services/AI/Resume/AgySessionStore.swift`
- Create: `Muxy/Services/AI/Resume/AgyResumeStrategy.swift`
- Test: `Tests/MuxyTests/Services/AI/Resume/AgySessionStoreTests.swift`

**Interfaces:**
- Consumes: seam protocols (Task 1), `AIProviderIntegration` (existing).
- Produces:
  - `struct AgyProvider: AIProviderIntegration, AgentResumeProviding` with `id = "agy"`, `displayName = "Antigravity"`, `socketTypeKey = "agy_hook"`, `iconName = "sparkles"`, `executableNames = ["agy"]`.
  - `struct AgySessionStore: AgentSessionStore` — reads `~/.gemini/antigravity-cli/cache/last_conversations.json` (a `{cwd: conversationUUID}` map); a directory matches if the map has a key equal to `directory` or `directory + "/"`; `id` = the mapped UUID; `updatedAt` = mtime of the map file.
  - `struct AgyResumeStrategy: AgentResumeStrategy`.

- [ ] **Step 0: Verify identity + resume flag (⚠)**

Run: `agy --help 2>&1 | grep -iE "resume|conversation|--session|continue"`
Confirm agy's resume flag. If it differs from `agy --resume <id>`, update Step 3 strings and Step 1 assertions. If agy has no resume flag, set both to `nil` and adjust the test.

- [ ] **Step 1: Write the failing test**

```swift
import Foundation
import Testing

@testable import Muxy

@Suite("AgySessionStore")
struct AgySessionStoreTests {
    @Test("resolves a conversation uuid from the cwd index map")
    func resolvesFromIndex() throws {
        let home = NSTemporaryDirectory() + "agy-store-" + UUID().uuidString
        let cacheDir = home + "/.gemini/antigravity-cli/cache"
        try FileManager.default.createDirectory(atPath: cacheDir, withIntermediateDirectories: true)
        let json = #"{"/Users/x/proj/":"conv-9","/other":"conv-1"}"#
        try json.write(toFile: cacheDir + "/last_conversations.json", atomically: true, encoding: .utf8)

        let store = AgySessionStore(homeDirectory: home)
        let sessions = store.sessions(inDirectory: "/Users/x/proj")

        #expect(sessions.count == 1)
        #expect(sessions.first?.id == "conv-9")
        #expect(sessions.first?.providerID == "agy")
    }

    @Test("resume strategy builds the resume command")
    func resumeStrategy() {
        #expect(AgyResumeStrategy().resumeCommand(sessionID: "conv-9", cwd: "/x") == "agy --resume conv-9")
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `swift test --filter AgySessionStoreTests 2>&1 | tail -20`
Expected: FAIL — `cannot find 'AgySessionStore' in scope`.

- [ ] **Step 3: Write minimal implementation**

`Muxy/Services/AI/Resume/AgyResumeStrategy.swift`:

```swift
import Foundation

struct AgyResumeStrategy: AgentResumeStrategy {
    func resumeCommand(sessionID: String?, cwd _: String) -> String? {
        guard let sessionID, !sessionID.isEmpty else { return continueLatestCommand }
        return "agy --resume \(sessionID)"
    }

    var continueLatestCommand: String? { nil }
}
```

`Muxy/Services/AI/Resume/AgySessionStore.swift`:

```swift
import Foundation

struct AgySessionStore: AgentSessionStore {
    private let homeDirectory: String

    init(homeDirectory: String = NSHomeDirectory()) {
        self.homeDirectory = homeDirectory
    }

    func sessions(inDirectory directory: String) -> [AgentSessionRef] {
        let path = homeDirectory + "/.gemini/antigravity-cli/cache/last_conversations.json"
        guard let data = FileManager.default.contents(atPath: path),
              let map = try? JSONSerialization.jsonObject(with: data) as? [String: String]
        else { return [] }
        let candidates = [directory, directory + "/"]
        guard let key = candidates.first(where: { map[$0] != nil }), let id = map[key] else { return [] }
        let attributes = try? FileManager.default.attributesOfItem(atPath: path)
        let updatedAt = (attributes?[.modificationDate] as? Date) ?? Date(timeIntervalSince1970: 0)
        return [AgentSessionRef(
            id: id, providerID: "agy", cwd: directory, gitBranch: nil,
            title: nil, preview: nil, updatedAt: updatedAt, pinned: false, archived: false)]
    }
}
```

`Muxy/Services/AI/Resume/AgyProvider.swift`:

```swift
import Foundation

struct AgyProvider: AIProviderIntegration, AgentResumeProviding {
    let id = "agy"
    let displayName = "Antigravity"
    let socketTypeKey = "agy_hook"
    let iconName = "sparkles"
    let executableNames = ["agy"]

    var resumeStrategy: AgentResumeStrategy? { AgyResumeStrategy() }
    var sessionStore: AgentSessionStore? { AgySessionStore() }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `swift test --filter AgySessionStoreTests 2>&1 | tail -20`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add Muxy/Services/AI/Resume Tests/MuxyTests/Services/AI/Resume/AgySessionStoreTests.swift
git commit -m "feat: add agy (Antigravity) provider, session store and resume strategy"
```

---

### Task 6: Hermes adapter — SQLite store + resume strategy

**Files:**
- Create: `Muxy/Services/AI/Resume/HermesProvider.swift`
- Create: `Muxy/Services/AI/Resume/HermesSessionStore.swift`
- Create: `Muxy/Services/AI/Resume/HermesResumeStrategy.swift`
- Create: `Muxy/Services/AI/Resume/SQLiteReader.swift` (tiny read-only wrapper over `libsqlite3`)
- Test: `Tests/MuxyTests/Services/AI/Resume/HermesSessionStoreTests.swift`

**Interfaces:**
- Consumes: seam protocols (Task 1).
- Produces:
  - `enum SQLiteReader { static func rows(databasePath: String, query: String, parameters: [String]) -> [[String: String]] }` — opens `SQLITE_OPEN_READONLY`, binds text params, returns each row as `[column: textValue]`; returns `[]` on any error.
  - `struct HermesSessionStore: AgentSessionStore` — queries `~/.hermes/state.db`: `SELECT id, title, cwd, git_branch, started_at, pinned, archived FROM sessions WHERE cwd = ? ORDER BY started_at DESC`; `updatedAt` = `Date(timeIntervalSince1970: started_at)`; `pinned`/`archived` from the integer columns.
  - `struct HermesProvider: AIProviderIntegration, AgentResumeProviding` with `id = "hermes"`, `displayName = "Hermes"`, `socketTypeKey = "hermes_hook"`, `iconName = "bolt"`, `executableNames = ["hermes"]`.
  - `struct HermesResumeStrategy: AgentResumeStrategy`.

- [ ] **Step 0: Verify resume flag + link libsqlite3 (⚠)**

Run: `hermes --help 2>&1 | grep -iE "resume|--session|continue"`
Confirm the resume flag; update strings/test if different, or set to `nil` if none.
Confirm `import SQLite3` compiles (macOS ships `libsqlite3.tbd`; SPM exposes it via the system module `SQLite3` with no Package.swift change). If the module is unavailable, fall back to shelling out to `/usr/bin/sqlite3` inside `SQLiteReader` (run `sqlite3 -json <db> <query>` via `Process`), keeping the same signature.

- [ ] **Step 1: Write the failing test**

```swift
import Foundation
import Testing

@testable import Muxy

@Suite("HermesSessionStore")
struct HermesSessionStoreTests {
    @Test("returns sessions for a cwd from state.db")
    func returnsSessions() throws {
        let home = NSTemporaryDirectory() + "hermes-store-" + UUID().uuidString
        try FileManager.default.createDirectory(atPath: home + "/.hermes", withIntermediateDirectories: true)
        let db = home + "/.hermes/state.db"
        let sql = """
        CREATE TABLE sessions (id TEXT, title TEXT, cwd TEXT, git_branch TEXT, started_at REAL, pinned INTEGER, archived INTEGER);
        INSERT INTO sessions VALUES ('s1','Fix bug','/Users/x/proj','main',1000.0,1,0);
        INSERT INTO sessions VALUES ('s2','Other','/nope','main',900.0,0,0);
        """
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/sqlite3")
        process.arguments = [db, sql]
        try process.run()
        process.waitUntilExit()

        let store = HermesSessionStore(homeDirectory: home)
        let sessions = store.sessions(inDirectory: "/Users/x/proj")

        #expect(sessions.count == 1)
        #expect(sessions.first?.id == "s1")
        #expect(sessions.first?.title == "Fix bug")
        #expect(sessions.first?.pinned == true)
    }

    @Test("resume strategy builds the resume command")
    func resumeStrategy() {
        #expect(HermesResumeStrategy().resumeCommand(sessionID: "s1", cwd: "/x") == "hermes --resume s1")
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `swift test --filter HermesSessionStoreTests 2>&1 | tail -20`
Expected: FAIL — `cannot find 'HermesSessionStore' in scope`.

- [ ] **Step 3: Write minimal implementation**

`Muxy/Services/AI/Resume/SQLiteReader.swift`:

```swift
import Foundation
import SQLite3

enum SQLiteReader {
    private static let transient = unsafeBitCast(-1, to: sqlite3_destructor_type.self)

    static func rows(databasePath: String, query: String, parameters: [String]) -> [[String: String]] {
        var database: OpaquePointer?
        guard sqlite3_open_v2(databasePath, &database, SQLITE_OPEN_READONLY, nil) == SQLITE_OK else {
            sqlite3_close(database)
            return []
        }
        defer { sqlite3_close(database) }

        var statement: OpaquePointer?
        guard sqlite3_prepare_v2(database, query, -1, &statement, nil) == SQLITE_OK else { return [] }
        defer { sqlite3_finalize(statement) }

        for (index, value) in parameters.enumerated() {
            sqlite3_bind_text(statement, Int32(index + 1), value, -1, transient)
        }

        var results: [[String: String]] = []
        while sqlite3_step(statement) == SQLITE_ROW {
            var row: [String: String] = [:]
            for column in 0 ..< sqlite3_column_count(statement) {
                guard let name = sqlite3_column_name(statement, column) else { continue }
                if let text = sqlite3_column_text(statement, column) {
                    row[String(cString: name)] = String(cString: text)
                }
            }
            results.append(row)
        }
        return results
    }
}
```

`Muxy/Services/AI/Resume/HermesResumeStrategy.swift`:

```swift
import Foundation

struct HermesResumeStrategy: AgentResumeStrategy {
    func resumeCommand(sessionID: String?, cwd _: String) -> String? {
        guard let sessionID, !sessionID.isEmpty else { return continueLatestCommand }
        return "hermes --resume \(sessionID)"
    }

    var continueLatestCommand: String? { nil }
}
```

`Muxy/Services/AI/Resume/HermesSessionStore.swift`:

```swift
import Foundation

struct HermesSessionStore: AgentSessionStore {
    private let homeDirectory: String

    init(homeDirectory: String = NSHomeDirectory()) {
        self.homeDirectory = homeDirectory
    }

    func sessions(inDirectory directory: String) -> [AgentSessionRef] {
        let path = homeDirectory + "/.hermes/state.db"
        guard FileManager.default.fileExists(atPath: path) else { return [] }
        let query = """
        SELECT id, title, cwd, git_branch, started_at, pinned, archived \
        FROM sessions WHERE cwd = ? ORDER BY started_at DESC
        """
        return SQLiteReader.rows(databasePath: path, query: query, parameters: [directory]).compactMap { row in
            guard let id = row["id"] else { return nil }
            let seconds = Double(row["started_at"] ?? "0") ?? 0
            return AgentSessionRef(
                id: id, providerID: "hermes", cwd: directory, gitBranch: row["git_branch"],
                title: row["title"], preview: row["title"],
                updatedAt: Date(timeIntervalSince1970: seconds),
                pinned: row["pinned"] == "1", archived: row["archived"] == "1")
        }
    }
}
```

`Muxy/Services/AI/Resume/HermesProvider.swift`:

```swift
import Foundation

struct HermesProvider: AIProviderIntegration, AgentResumeProviding {
    let id = "hermes"
    let displayName = "Hermes"
    let socketTypeKey = "hermes_hook"
    let iconName = "bolt"
    let executableNames = ["hermes"]

    var resumeStrategy: AgentResumeStrategy? { HermesResumeStrategy() }
    var sessionStore: AgentSessionStore? { HermesSessionStore() }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `swift test --filter HermesSessionStoreTests 2>&1 | tail -20`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add Muxy/Services/AI/Resume Tests/MuxyTests/Services/AI/Resume/HermesSessionStoreTests.swift
git commit -m "feat: add Hermes provider, sqlite session store and resume strategy"
```

---

### Task 7: Register agy + Hermes and expose resume lookup on the registry

**Files:**
- Modify: `Muxy/Services/AI/AIProviderIntegration.swift` (add the two providers to `AIProviderRegistry`, add a lookup helper)
- Test: `Tests/MuxyTests/Services/AI/Resume/AIProviderRegistryResumeTests.swift`

**Interfaces:**
- Consumes: all providers (Tasks 2–6).
- Produces on `AIProviderRegistry`:
  - `func resumeProvider(forProviderID id: String) -> AgentResumeProviding?` — returns the registered provider cast to `AgentResumeProviding`, or `nil`.

- [ ] **Step 1: Write the failing test**

```swift
import Foundation
import Testing

@testable import Muxy

@Suite("AIProviderRegistry resume lookup")
struct AIProviderRegistryResumeTests {
    @Test("claude resolves a resume strategy")
    func claudeResume() {
        let registry = AIProviderRegistry.shared
        let provider = registry.resumeProvider(forProviderID: "claude")
        #expect(provider?.resumeStrategy?.resumeCommand(sessionID: "x", cwd: "/p") == "claude --resume x")
    }

    @Test("agy and hermes are registered")
    func newProvidersRegistered() {
        let registry = AIProviderRegistry.shared
        #expect(registry.resumeProvider(forProviderID: "agy") != nil)
        #expect(registry.resumeProvider(forProviderID: "hermes") != nil)
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `swift test --filter AIProviderRegistryResumeTests 2>&1 | tail -20`
Expected: FAIL — `value of type 'AIProviderRegistry' has no member 'resumeProvider'`.

- [ ] **Step 3: Write minimal implementation**

In `AIProviderRegistry`, add the two stored providers next to the existing ones:

```swift
    private let agyProvider = AgyProvider()
    private let hermesProvider = HermesProvider()
```

Add them to the `providers` lazy array:

```swift
    lazy var providers: [AIProviderIntegration] = injectedProviders ?? [
        claudeCodeProvider,
        openCodeProvider,
        codexProvider,
        cursorProvider,
        droidProvider,
        piProvider,
        grokProvider,
        agyProvider,
        hermesProvider,
    ]
```

Add the lookup helper as a method on `AIProviderRegistry`:

```swift
    func resumeProvider(forProviderID id: String) -> AgentResumeProviding? {
        providers.first { $0.id == id } as? AgentResumeProviding
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `swift test --filter AIProviderRegistryResumeTests 2>&1 | tail -20`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add Muxy/Services/AI/AIProviderIntegration.swift Tests/MuxyTests/Services/AI/Resume/AIProviderRegistryResumeTests.swift
git commit -m "feat: register agy and hermes providers with resume lookup"
```

---

### Task 8: Resume resolution service (the degradation ladder)

**Files:**
- Create: `Muxy/Services/AI/Resume/AgentResumeResolver.swift`
- Test: `Tests/MuxyTests/Services/AI/Resume/AgentResumeResolverTests.swift`

**Interfaces:**
- Consumes: `AgentResumeProviding` (Task 1), `AIProviderRegistry.resumeProvider` (Task 7).
- Produces:
  - `enum AgentResumeResolver { static func command(providerID: String, sessionID: String?, cwd: String, registry: AIProviderRegistry = .shared) -> String? }`
  - Ladder: no provider → `nil`; explicit `sessionID` → `strategy.resumeCommand(sessionID:cwd:)`; else newest `sessionStore.sessions(inDirectory:).first` → `strategy.resumeCommand(sessionID: ref.id, cwd:)`; else `strategy.resumeCommand(sessionID: nil, cwd:)` (continue-latest); else `nil`.

- [ ] **Step 1: Write the failing test**

```swift
import Foundation
import Testing

@testable import Muxy

@Suite("AgentResumeResolver")
struct AgentResumeResolverTests {
    private struct StubStrategy: AgentResumeStrategy {
        func resumeCommand(sessionID: String?, cwd _: String) -> String? {
            guard let sessionID else { return "tool --continue" }
            return "tool --resume \(sessionID)"
        }
        var continueLatestCommand: String? { "tool --continue" }
    }

    private struct StubStore: AgentSessionStore {
        let refs: [AgentSessionRef]
        func sessions(inDirectory _: String) -> [AgentSessionRef] { refs }
    }

    private struct StubProvider: AIProviderIntegration, AgentResumeProviding {
        let id = "tool"
        let displayName = "Tool"
        let socketTypeKey = "tool"
        let iconName = "x"
        let executableNames = ["tool"]
        let store: [AgentSessionRef]
        var resumeStrategy: AgentResumeStrategy? { StubStrategy() }
        var sessionStore: AgentSessionStore? { StubStore(refs: store) }
    }

    private func registry(store: [AgentSessionRef]) -> AIProviderRegistry {
        AIProviderRegistry(providers: [StubProvider(store: store)])
    }

    @Test("explicit session id wins")
    func explicitID() {
        let command = AgentResumeResolver.command(
            providerID: "tool", sessionID: "abc", cwd: "/p", registry: registry(store: []))
        #expect(command == "tool --resume abc")
    }

    @Test("falls back to newest discovered session")
    func discovered() {
        let ref = AgentSessionRef(id: "disc", providerID: "tool", cwd: "/p", gitBranch: nil,
            title: nil, preview: nil, updatedAt: Date(timeIntervalSince1970: 5), pinned: false, archived: false)
        let command = AgentResumeResolver.command(
            providerID: "tool", sessionID: nil, cwd: "/p", registry: registry(store: [ref]))
        #expect(command == "tool --resume disc")
    }

    @Test("falls back to continue-latest when nothing discovered")
    func continueLatest() {
        let command = AgentResumeResolver.command(
            providerID: "tool", sessionID: nil, cwd: "/p", registry: registry(store: []))
        #expect(command == "tool --continue")
    }

    @Test("unknown provider yields nil")
    func unknown() {
        let command = AgentResumeResolver.command(
            providerID: "ghost", sessionID: nil, cwd: "/p", registry: registry(store: []))
        #expect(command == nil)
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `swift test --filter AgentResumeResolverTests 2>&1 | tail -20`
Expected: FAIL — `cannot find 'AgentResumeResolver' in scope`.

- [ ] **Step 3: Write minimal implementation**

```swift
import Foundation

enum AgentResumeResolver {
    static func command(
        providerID: String,
        sessionID: String?,
        cwd: String,
        registry: AIProviderRegistry = .shared
    ) -> String? {
        guard let provider = registry.resumeProvider(forProviderID: providerID),
              let strategy = provider.resumeStrategy
        else { return nil }

        if let sessionID, !sessionID.isEmpty {
            return strategy.resumeCommand(sessionID: sessionID, cwd: cwd)
        }
        if let discovered = provider.sessionStore?.sessions(inDirectory: cwd).first {
            return strategy.resumeCommand(sessionID: discovered.id, cwd: cwd)
        }
        return strategy.resumeCommand(sessionID: nil, cwd: cwd)
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `swift test --filter AgentResumeResolverTests 2>&1 | tail -20`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add Muxy/Services/AI/Resume/AgentResumeResolver.swift Tests/MuxyTests/Services/AI/Resume/AgentResumeResolverTests.swift
git commit -m "feat: add agent resume resolver with degradation ladder"
```

---

### Task 9: Persist agent session on the tab snapshot

**Files:**
- Modify: `Muxy/Models/Workspace/WorkspaceSnapshot.swift` (add `agentSession` to `TerminalTabSnapshot` — struct property, init param, `CodingKeys`, `init(from:)`)
- Create: `Muxy/Models/Workspace/AgentSessionSnapshot.swift`
- Modify: `Muxy/Models/Terminal/TerminalTab.swift` (`snapshot()` sets `agentSession`; keep `init(restoring:)` unchanged)
- Test: `Tests/MuxyTests/Models/Workspace/TerminalTabSnapshotAgentSessionTests.swift`

**Interfaces:**
- Consumes: nothing from other tasks (pure data).
- Produces:
  - `struct AgentSessionSnapshot: Codable, Equatable { let providerID: String; let sessionID: String?; let cwd: String }`
  - `TerminalTabSnapshot.agentSession: AgentSessionSnapshot?` — optional, additive; `init` gains `agentSession: AgentSessionSnapshot? = nil` as the last parameter; decoded via `decodeIfPresent`.

- [ ] **Step 1: Write the failing test**

```swift
import Foundation
import Testing

@testable import Muxy

@Suite("TerminalTabSnapshot agentSession")
struct TerminalTabSnapshotAgentSessionTests {
    @Test("round-trips an agent session")
    func roundTrips() throws {
        let snapshot = TerminalTabSnapshot(
            kind: .terminal, customTitle: nil, colorID: nil, isPinned: false,
            projectPath: "/p", paneTitle: "T",
            agentSession: AgentSessionSnapshot(providerID: "claude", sessionID: "sid", cwd: "/p"))
        let data = try JSONEncoder().encode(snapshot)
        let decoded = try JSONDecoder().decode(TerminalTabSnapshot.self, from: data)
        #expect(decoded.agentSession?.providerID == "claude")
        #expect(decoded.agentSession?.sessionID == "sid")
    }

    @Test("legacy snapshot without agentSession still decodes")
    func legacyDecodes() throws {
        let json = #"{"kind":"terminal","id":"\#(UUID().uuidString)","isPinned":false,"projectPath":"/p","paneTitle":"T"}"#
        let decoded = try JSONDecoder().decode(TerminalTabSnapshot.self, from: Data(json.utf8))
        #expect(decoded.agentSession == nil)
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `swift test --filter TerminalTabSnapshotAgentSessionTests 2>&1 | tail -20`
Expected: FAIL — `extra argument 'agentSession' in call`.

- [ ] **Step 3: Write minimal implementation**

Create `Muxy/Models/Workspace/AgentSessionSnapshot.swift`:

```swift
import Foundation

struct AgentSessionSnapshot: Codable, Equatable {
    let providerID: String
    let sessionID: String?
    let cwd: String
}
```

In `WorkspaceSnapshot.swift`, in `TerminalTabSnapshot`:
- Add the stored property after `browserProfileID`: `let agentSession: AgentSessionSnapshot?`
- Add init parameter as the final argument: `agentSession: AgentSessionSnapshot? = nil` and assign `self.agentSession = agentSession`.
- Add `case agentSession` to `CodingKeys`.
- In `init(from:)` add: `agentSession = try container.decodeIfPresent(AgentSessionSnapshot.self, forKey: .agentSession)`.

In `TerminalTab.swift` `snapshot()`, add the trailing argument:

```swift
            browserProfileID: content.browserState?.profileID.uuidString,
            agentSession: agentSessionSnapshot
```

Add a private computed helper on `TerminalTab`:

```swift
    private var agentSessionSnapshot: AgentSessionSnapshot? {
        guard let paneID = content.pane?.id,
              let providerID = DetectedAgentStore.shared.agent(for: paneID),
              let cwd = content.pane?.currentWorkingDirectory
        else { return nil }
        return AgentSessionSnapshot(providerID: providerID, sessionID: nil, cwd: cwd)
    }
```

(The `sessionID` stays `nil` here; Task 11 fills it from the registry when hook capture lands. Discovery in the resolver covers the `nil` case meanwhile.)

- [ ] **Step 4: Run test to verify it passes**

Run: `swift test --filter TerminalTabSnapshotAgentSessionTests 2>&1 | tail -20`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add Muxy/Models/Workspace/WorkspaceSnapshot.swift Muxy/Models/Workspace/AgentSessionSnapshot.swift Muxy/Models/Terminal/TerminalTab.swift Tests/MuxyTests/Models/Workspace/TerminalTabSnapshotAgentSessionTests.swift
git commit -m "feat: persist agent session on terminal tab snapshot"
```

---

### Task 10: `agent.resolveResume` API verb + `agent.session` event

**Files:**
- Modify: `Muxy/Models/Extension/ExtensionEvent.swift` (add `agentSession` event name)
- Create: `Muxy/Services/Extensions/AgentSessionEventEmitter.swift`
- Modify: `Muxy/Services/MuxyAPI/MuxyAPIDispatcher.swift` (add `case "agent.resolveResume"`)
- Test: `Tests/MuxyTests/Services/MuxyAPI/MuxyAPIResolveResumeTests.swift`

**Interfaces:**
- Consumes: `AgentResumeResolver.command` (Task 8), `ExtensionEvent` broadcast pattern.
- Produces:
  - `ExtensionEventName.agentSession = "agent.session"`.
  - `enum AgentSessionEventEmitter { static func emit(paneID: UUID, providerID: String, sessionID: String?, cwd: String) }` broadcasting `ExtensionEvent(name: .agentSession, payload: ["paneID","providerID","sessionID","cwd"])`.
  - Dispatcher verb `agent.resolveResume` — args `{providerID: String, cwd: String, sessionID?: String}` → `{"command": String?}` (JSON), running `AgentResumeResolver.command(...)`.

- [ ] **Step 1: Write the failing test**

```swift
import Foundation
import Testing

@testable import Muxy

@Suite("agent.resolveResume verb")
struct MuxyAPIResolveResumeTests {
    @Test("resolves a command for a known provider with explicit id")
    func resolvesCommand() {
        let command = AgentResumeResolver.command(
            providerID: "claude", sessionID: "sid", cwd: "/p")
        #expect(command == "claude --resume sid")
    }

    @Test("agent.session event name is stable")
    func eventName() {
        #expect(ExtensionEventName.agentSession == "agent.session")
    }
}
```

(The dispatcher `case` is exercised by the resolver + event-name assertions above; the verb wiring is a thin adapter over `AgentResumeResolver`, verified end-to-end in Task 11's E2E.)

- [ ] **Step 2: Run test to verify it fails**

Run: `swift test --filter MuxyAPIResolveResumeTests 2>&1 | tail -20`
Expected: FAIL — `type 'ExtensionEventName' has no member 'agentSession'`.

- [ ] **Step 3: Write minimal implementation**

In `ExtensionEvent.swift` add to `ExtensionEventName`:

```swift
    static let agentSession = "agent.session"
```

Create `Muxy/Services/Extensions/AgentSessionEventEmitter.swift`:

```swift
import Foundation

enum AgentSessionEventEmitter {
    static func emit(paneID: UUID, providerID: String, sessionID: String?, cwd: String) {
        NotificationSocketServer.shared.broadcast(event: ExtensionEvent(
            name: ExtensionEventName.agentSession,
            payload: [
                "paneID": paneID.uuidString,
                "providerID": providerID,
                "sessionID": sessionID ?? "",
                "cwd": cwd,
            ]))
    }
}
```

In `MuxyAPIDispatcher.swift`, next to `case "tabs.open":`, add:

```swift
        case "agent.resolveResume":
            let providerID = (args["providerID"] as? String) ?? ""
            let cwd = (args["cwd"] as? String) ?? ""
            let sessionID = args["sessionID"] as? String
            let command = AgentResumeResolver.command(providerID: providerID, sessionID: sessionID, cwd: cwd)
            return ["command": command as Any]
```

(Match the surrounding handler's return convention — the existing verbs in this switch return dictionaries/values that the dispatcher serializes. Follow the exact style of the neighboring `case "tabs.list":` return.)

- [ ] **Step 4: Run test to verify it passes**

Run: `swift test --filter MuxyAPIResolveResumeTests 2>&1 | tail -20`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add Muxy/Models/Extension/ExtensionEvent.swift Muxy/Services/Extensions/AgentSessionEventEmitter.swift Muxy/Services/MuxyAPI/MuxyAPIDispatcher.swift Tests/MuxyTests/Services/MuxyAPI/MuxyAPIResolveResumeTests.swift
git commit -m "feat: add agent.resolveResume verb and agent.session event"
```

---

### Task 11: Global opt-in setting + emit agent.session on detection

**Files:**
- Modify: `Muxy/Models/Preferences/NotificationSettings.swift` OR create `Muxy/Models/Preferences/SessionRestorePreferences.swift` (follow the existing single-key preference pattern)
- Modify: the call site that sets `DetectedAgentStore.shared.setAgent(...)` (find with grep in Step 3) to also emit `AgentSessionEventEmitter.emit(...)`
- Modify: `Muxy/Views/Settings/AISettingsView.swift` (add the toggle row)
- Test: `Tests/MuxyTests/Models/Preferences/SessionRestorePreferencesTests.swift`

**Interfaces:**
- Consumes: `AgentSessionEventEmitter` (Task 10), `DetectedAgentStore` (existing).
- Produces:
  - `enum SessionRestorePreferences { static func autoResumeEnabled(defaults: UserDefaults = .standard) -> Bool; static func setAutoResumeEnabled(_ value: Bool, defaults: UserDefaults = .standard); static let autoResumeKey = "muxy.sessionRestore.autoResume"; static let autoResumeDefault = false }`

- [ ] **Step 1: Write the failing test**

```swift
import Foundation
import Testing

@testable import Muxy

@Suite("SessionRestorePreferences")
struct SessionRestorePreferencesTests {
    private func defaults() -> UserDefaults {
        let suite = UserDefaults(suiteName: "restore-" + UUID().uuidString)!
        return suite
    }

    @Test("defaults to disabled")
    func defaultDisabled() {
        #expect(SessionRestorePreferences.autoResumeEnabled(defaults: defaults()) == false)
    }

    @Test("persists an enabled value")
    func persists() {
        let store = defaults()
        SessionRestorePreferences.setAutoResumeEnabled(true, defaults: store)
        #expect(SessionRestorePreferences.autoResumeEnabled(defaults: store) == true)
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `swift test --filter SessionRestorePreferencesTests 2>&1 | tail -20`
Expected: FAIL — `cannot find 'SessionRestorePreferences' in scope`.

- [ ] **Step 3: Write minimal implementation**

Create `Muxy/Models/Preferences/SessionRestorePreferences.swift`:

```swift
import Foundation

enum SessionRestorePreferences {
    static let autoResumeKey = "muxy.sessionRestore.autoResume"
    static let autoResumeDefault = false

    static func autoResumeEnabled(defaults: UserDefaults = .standard) -> Bool {
        defaults.object(forKey: autoResumeKey) == nil
            ? autoResumeDefault
            : defaults.bool(forKey: autoResumeKey)
    }

    static func setAutoResumeEnabled(_ value: Bool, defaults: UserDefaults = .standard) {
        defaults.set(value, forKey: autoResumeKey)
    }
}
```

Find the detection call site and emit the event next to it:

```bash
grep -rn "DetectedAgentStore.shared.setAgent" Muxy
```

At that call site, after `setAgent(providerID, for: paneID)`, when `providerID` is non-nil and the pane has a working directory, call:

```swift
if let providerID, let cwd = <paneWorkingDirectoryExpressionInScope> {
    AgentSessionEventEmitter.emit(paneID: paneID, providerID: providerID, sessionID: nil, cwd: cwd)
}
```

Add the toggle to `AISettingsView.swift`, following the existing `Toggle`/row pattern in that view:

```swift
Toggle("Auto-resume AI sessions on restore", isOn: Binding(
    get: { SessionRestorePreferences.autoResumeEnabled() },
    set: { SessionRestorePreferences.setAutoResumeEnabled($0) }))
```

- [ ] **Step 4: Run test to verify it passes**

Run: `swift test --filter SessionRestorePreferencesTests 2>&1 | tail -20`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add Muxy/Models/Preferences/SessionRestorePreferences.swift Muxy/Views/Settings/AISettingsView.swift Tests/MuxyTests/Models/Preferences/SessionRestorePreferencesTests.swift
git commit -m "feat: add auto-resume opt-in setting and emit agent.session on detection"
```

---

### Task 12: Bundled `session-restore` extension + SKILL/docs

**Files:**
- Create: `~/.config/muxy/extensions/demo-session-restore/manifest.json`
- Create: `~/.config/muxy/extensions/demo-session-restore/main.js`
- Modify: `Muxy/Resources/skills/muxy-extension/SKILL.md` (document `agent.session` + `agent.resolveResume`)
- Modify: `docs/` extension reference page covering events + verbs (find with grep in Step 1)

**Interfaces:**
- Consumes: `agent.session` event, `agent.resolveResume` verb, `tabs.open` verb, `tab.closed`/`tab.created` events.
- Produces: a working reference extension that, when auto-resume is on, reopens a resumed agent tab.

- [ ] **Step 1: Locate the docs to update**

Run: `grep -rln "tab.created\|tabs.open\|agent.status" Muxy/Resources/skills/muxy-extension docs`
Note the SKILL.md event/verb tables to extend.

- [ ] **Step 2: Write the extension**

`~/.config/muxy/extensions/demo-session-restore/manifest.json`:

```json
{
  "id": "demo-session-restore",
  "name": "Demo Session Restore",
  "version": "0.1.0",
  "events": ["agent.session", "tab.closed", "tab.created"],
  "permissions": ["tabs.open"]
}
```

`main.js` (follow the existing demo extensions' API shape in `~/.config/muxy/extensions/*`): maintain an in-memory map `paneID → {providerID, sessionID, cwd}` from `agent.session`; on a restored `tab.created` for a pane that had a session, call `agent.resolveResume({providerID, sessionID, cwd})` and, if a command comes back, `tabs.open({directory: cwd, command})`.

- [ ] **Step 3: Update SKILL + docs**

Add `agent.session` (payload: paneID, providerID, sessionID, cwd) to the events table and `agent.resolveResume` (args: providerID, cwd, sessionID?; returns `{command}`) to the verbs table in `SKILL.md` and the docs page found in Step 1. Keep entries consistent with neighboring rows.

- [ ] **Step 4: Manual verification**

Enable the toggle (Task 11). In a project tab, run `claude` and hold a short conversation. Quit Muxy. Relaunch. Expected: the restored tab runs `claude --resume <id>` (or `--continue`) and the conversation reattaches. Screenshot for the PR. Repeat with `codex`.

- [ ] **Step 5: Commit**

```bash
git add Muxy/Resources/skills/muxy-extension/SKILL.md docs
git commit -m "docs: document agent.session event and agent.resolveResume verb"
```

(The extension files under `~/.config/muxy/extensions` are outside the repo; note them in the PR body as the demo to install.)

---

### Task 13: Hook-reported session id capture (enhancement)

**Files:**
- Modify: the hook ingestion path (locate in Step 1) to route `session_id` + `cwd` from `SessionStart` hook payloads into `DetectedAgentStore`/registry so snapshots carry a real `sessionID`
- Test: `Tests/MuxyTests/Services/AI/Resume/HookSessionCaptureTests.swift`

**Interfaces:**
- Consumes: existing hook bridge, `AgentSessionEventEmitter` (Task 10).
- Produces: when a `SessionStart` hook fires, the pane's `AgentSessionSnapshot.sessionID` is the tool's real session id (not `nil`), making restore exact without discovery.

- [ ] **Step 1: Locate the hook payload → pane mapping**

Run:
```bash
grep -rn "session_id\|sessionId\|SessionStart\|hook" Muxy/Services/Socket MuxyHookKit MuxyHookBridge | grep -iv test | head -40
```
Identify where an incoming hook event is associated with a pane and where its JSON payload is parsed. Document the exact type/function in the commit message.

- [ ] **Step 2: Write the failing test**

```swift
import Foundation
import Testing

@testable import Muxy

@Suite("Hook session capture")
struct HookSessionCaptureTests {
    @Test("extracts session id and cwd from a SessionStart payload")
    func extractsFields() {
        let payload: [String: Any] = ["hook_event_name": "SessionStart",
                                      "session_id": "hook-sid", "cwd": "/Users/x/proj"]
        let parsed = HookSessionCapture.parse(payload: payload)
        #expect(parsed?.sessionID == "hook-sid")
        #expect(parsed?.cwd == "/Users/x/proj")
    }

    @Test("ignores non-session hook events")
    func ignoresOthers() {
        #expect(HookSessionCapture.parse(payload: ["hook_event_name": "Stop"]) == nil)
    }
}
```

- [ ] **Step 3: Write minimal implementation**

Create `Muxy/Services/AI/Resume/HookSessionCapture.swift`:

```swift
import Foundation

enum HookSessionCapture {
    struct Parsed: Equatable {
        let sessionID: String
        let cwd: String
    }

    static func parse(payload: [String: Any]) -> Parsed? {
        guard (payload["hook_event_name"] as? String) == "SessionStart",
              let sessionID = payload["session_id"] as? String,
              let cwd = payload["cwd"] as? String
        else { return nil }
        return Parsed(sessionID: sessionID, cwd: cwd)
    }
}
```

At the hook ingestion site found in Step 1, when `HookSessionCapture.parse` returns a value and the event maps to a `paneID`, store the real `sessionID` alongside the provider (extend `DetectedAgentStore` with a parallel `sessionIDs: [UUID: String]` map and a `setSession(_:for:)` setter, and read it in `TerminalTab.agentSessionSnapshot` in place of the `nil` from Task 9), then call `AgentSessionEventEmitter.emit(paneID:providerID:sessionID:cwd:)`.

- [ ] **Step 4: Run test to verify it passes**

Run: `swift test --filter HookSessionCaptureTests 2>&1 | tail -20`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add Muxy/Services/AI/Resume/HookSessionCapture.swift Muxy/Services/AgentDetection/DetectedAgentStore.swift Muxy/Models/Terminal/TerminalTab.swift Tests/MuxyTests/Services/AI/Resume/HookSessionCaptureTests.swift
git commit -m "feat: capture real session id from SessionStart hook payloads"
```

---

### Task 14: Full check + PR

**Files:** none (verification + integration).

- [ ] **Step 1: Full checks**

Run: `scripts/checks.sh --fix`
Expected: format, lint `--strict`, build, and all tests pass. Fix anything it flags.

- [ ] **Step 2: Coverage gate (opt-in)**

Run: `scripts/checks.sh --coverage`
Expected: passes. If new files drop coverage, add missing-branch tests (e.g. store-missing-folder returns `[]`).

- [ ] **Step 3: Manual E2E**

Toggle on. `claude` in project A, `codex` in project B, quit, relaunch → both reattach. Screenshot/record.

- [ ] **Step 4: Push + PR**

```bash
git push -u origin feat/ai-session-restore
gh pr create --title "AI session restore: reattach real agent sessions on restore" --body "$(cat <<'EOF'
Reattaches real AI CLI sessions on workspace restore, gated behind a global opt-in.
Core resolves the resume command per provider; a bundled extension executes it via tabs.open.
Demo: install ~/.config/muxy/extensions/demo-session-restore.
EOF
)"
```

---

## Self-Review

**Spec coverage:**
- Hybrid split → Tasks 1,8,10 (core seam + resolver + verb/event) and Task 12 (extension). ✓
- Auto-resume + global opt-in → Task 11. ✓
- All-providers pluggable → Tasks 2–7 (Claude, Codex, Cursor, agy, Hermes) + registry. ✓
- Capture-id-fallback-latest ladder → Task 8. ✓
- Five-agent store matrix (FolderGlob/RolloutScan/Index/SQLite) → Tasks 2–6. ✓
- Additive snapshot field → Task 9. ✓
- `agent.session` event + `agent.resolveResume` verb → Task 10. ✓
- Hook-reported id (primary capture) → Task 13. ✓
- Store-drift isolation / fail-soft → each adapter returns `[]`/`nil`; asserted by "empty when missing" tests. ✓
- Docs/SKILL + demo extension → Task 12. ✓
- Consent allow-listing (only resolver-produced commands) → enforced by Task 10 verb + Task 12 extension calling only `agent.resolveResume`. ✓

**Placeholder scan:** ⚠ markers are explicit verification steps (Tasks 4/5/6 Step 0), not logic placeholders. No "TBD"/"add error handling"/"similar to Task N". Each adapter task carries its own full code.

**Type consistency:** `AgentSessionRef`, `AgentResumeStrategy.resumeCommand(sessionID:cwd:)`, `AgentSessionStore.sessions(inDirectory:)`, `AgentResumeProviding.resumeStrategy`/`sessionStore`, `AgentResumeResolver.command(providerID:sessionID:cwd:registry:)`, `AgentSessionSnapshot(providerID:sessionID:cwd:)`, `AIProviderRegistry.resumeProvider(forProviderID:)`, `AgentSessionEventEmitter.emit(paneID:providerID:sessionID:cwd:)`, `SessionRestorePreferences.autoResumeEnabled(defaults:)` — used identically across tasks. ✓

**Known runtime-verify points (not guesses baked as fact):** Cursor/agy/Hermes resume flags (Tasks 4/5/6 Step 0); `SQLite3` module availability (Task 6 Step 0); hook payload field names + pane mapping (Task 13 Step 1); the exact detection call site + pane cwd expression (Task 11 Step 3). Each is an explicit step with a command, not an assumption.
