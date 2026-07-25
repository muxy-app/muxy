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
        #expect(AgyResumeStrategy().resumeCommand(sessionID: "conv-9", cwd: "/x") == "agy --conversation conv-9")
    }
}
