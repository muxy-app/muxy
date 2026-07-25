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
