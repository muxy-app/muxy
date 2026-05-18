import Foundation
import Testing

@testable import Muxy

@Suite("TerminalSessionFile")
struct TerminalSessionFileTests {
    @Test("Decodes v1 files without closed terminal tabs")
    func decodesV1WithoutClosedTerminalTabs() throws {
        let json = """
        {
          "schemaVersion": 1,
          "sessions": []
        }
        """
        let file = try JSONDecoder().decode(TerminalSessionFile.self, from: Data(json.utf8))
        #expect(file.schemaVersion == 1)
        #expect(file.sessions.isEmpty)
        #expect(file.closedTerminalTabs.isEmpty)
    }

    @Test("Closed terminal tab prefers last submitted command")
    func closedTerminalTabPrefersLastSubmittedCommand() {
        let snapshot = makeClosedSnapshot(
            startupCommand: "npm run dev",
            lastSubmittedCommand: "nvim Package.swift"
        )
        #expect(snapshot.commandToRestore == "nvim Package.swift")
    }

    @Test("Closed terminal tab falls back to startup command")
    func closedTerminalTabFallsBackToStartupCommand() {
        let snapshot = makeClosedSnapshot(
            startupCommand: "npm run dev",
            lastSubmittedCommand: nil
        )
        #expect(snapshot.commandToRestore == "npm run dev")
    }

    private func makeClosedSnapshot(
        startupCommand: String?,
        lastSubmittedCommand: String?
    ) -> ClosedTerminalTabSnapshot {
        ClosedTerminalTabSnapshot(
            id: UUID(),
            projectID: UUID(),
            worktreeID: UUID(),
            areaID: UUID(),
            projectPath: "/tmp/project",
            title: "Terminal",
            customTitle: nil,
            colorID: nil,
            workingDirectory: "/tmp/project",
            startupCommand: startupCommand,
            lastSubmittedCommand: lastSubmittedCommand,
            closedSequence: 1,
            closedAt: Date()
        )
    }
}
