import Foundation
import Testing

@testable import Muxy

@MainActor
@Suite("TerminalSessionStore")
struct TerminalSessionStoreTests {
    @Test("Retains max snapshots per worktree")
    func retainsMaxSnapshotsPerWorktree() {
        let projectA = UUID()
        let projectB = UUID()
        let worktreeA = UUID()
        let worktreeB = UUID()
        let worktreeC = UUID()
        let snapshots = [
            makeSnapshot(projectID: projectA, worktreeID: worktreeA, secondsAgo: 1),
            makeSnapshot(projectID: projectA, worktreeID: worktreeA, secondsAgo: 2),
            makeSnapshot(projectID: projectA, worktreeID: worktreeA, secondsAgo: 3),
            makeSnapshot(projectID: projectA, worktreeID: worktreeB, secondsAgo: 4),
            makeSnapshot(projectID: projectA, worktreeID: worktreeB, secondsAgo: 5),
            makeSnapshot(projectID: projectA, worktreeID: worktreeB, secondsAgo: 6),
            makeSnapshot(projectID: projectB, worktreeID: worktreeC, secondsAgo: 7),
            makeSnapshot(projectID: projectB, worktreeID: worktreeC, secondsAgo: 8),
            makeSnapshot(projectID: projectB, worktreeID: worktreeC, secondsAgo: 9),
        ]

        let retained = TerminalSessionStore.retainedSnapshots(snapshots, maxPerWorktree: 2)
        let counts = Dictionary(grouping: retained) { snapshot in
            WorktreeKey(projectID: snapshot.projectID, worktreeID: snapshot.worktreeID)
        }
        .mapValues(\.count)

        #expect(retained.count == 6)
        #expect(counts[WorktreeKey(projectID: projectA, worktreeID: worktreeA)] == 2)
        #expect(counts[WorktreeKey(projectID: projectA, worktreeID: worktreeB)] == 2)
        #expect(counts[WorktreeKey(projectID: projectB, worktreeID: worktreeC)] == 2)
        #expect(!retained.contains { $0.capturedAt == snapshots[2].capturedAt })
        #expect(!retained.contains { $0.capturedAt == snapshots[5].capturedAt })
        #expect(!retained.contains { $0.capturedAt == snapshots[8].capturedAt })
    }

    @Test("Pops selected closed terminal tab")
    func popsSelectedClosedTerminalTab() {
        let projectID = UUID()
        let worktreeID = UUID()
        let store = TerminalSessionStore(fileURL: temporaryFileURL())
        let first = makeClosedSnapshot(projectID: projectID, worktreeID: worktreeID, title: "First", sequence: 1)
        let second = makeClosedSnapshot(projectID: projectID, worktreeID: worktreeID, title: "Second", sequence: 2)

        store.recordClosedTerminalTab(first)
        store.recordClosedTerminalTab(second)

        let popped = store.popClosedTerminalTab(id: first.id, projectID: projectID, worktreeID: worktreeID)

        #expect(popped == first)
        #expect(store.closedTerminalTabs.map(\.id) == [second.id])
    }

    private func makeSnapshot(
        projectID: UUID,
        worktreeID: UUID,
        secondsAgo: TimeInterval
    ) -> TerminalSessionSnapshot {
        TerminalSessionSnapshot(
            id: UUID(),
            projectID: projectID,
            worktreeID: worktreeID,
            paneID: UUID(),
            tabID: UUID(),
            areaID: UUID(),
            projectPath: "/tmp/project",
            title: "Terminal",
            workingDirectory: "/tmp/project",
            startupCommand: nil,
            lastSubmittedCommand: nil,
            activity: .idle,
            capturedAt: Date(timeIntervalSinceNow: -secondsAgo)
        )
    }

    private func makeClosedSnapshot(
        projectID: UUID,
        worktreeID: UUID,
        title: String,
        sequence: Int64
    ) -> ClosedTerminalTabSnapshot {
        ClosedTerminalTabSnapshot(
            id: UUID(),
            projectID: projectID,
            worktreeID: worktreeID,
            areaID: UUID(),
            projectPath: "/tmp/project",
            title: title,
            customTitle: nil,
            colorID: nil,
            workingDirectory: "/tmp/project",
            startupCommand: nil,
            lastSubmittedCommand: nil,
            closedSequence: sequence,
            closedAt: Date(timeIntervalSince1970: TimeInterval(sequence))
        )
    }

    private func temporaryFileURL() -> URL {
        FileManager.default.temporaryDirectory
            .appendingPathComponent("terminal-sessions-\(UUID().uuidString).json")
    }
}
