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
