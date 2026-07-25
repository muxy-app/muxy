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
