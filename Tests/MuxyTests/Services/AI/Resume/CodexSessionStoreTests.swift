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
