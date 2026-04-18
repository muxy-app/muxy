import Foundation
import Testing

@testable import Muxy

@Suite("WorkspaceSnapshot")
@MainActor
struct WorkspaceSnapshotTests {
    private let testPath = "/tmp/test"

    @Test("TerminalTabSnapshot Codable round-trip for terminal")
    func terminalTabSnapshotRoundTrip() throws {
        let snapshot = TerminalTabSnapshot(
            kind: .terminal,
            customTitle: "My Tab",
            colorID: "blue",
            isPinned: true,
            projectPath: testPath,
            paneTitle: "Shell"
        )
        let data = try JSONEncoder().encode(snapshot)
        let decoded = try JSONDecoder().decode(TerminalTabSnapshot.self, from: data)

        #expect(decoded.kind == .terminal)
        #expect(decoded.customTitle == "My Tab")
        #expect(decoded.colorID == "blue")
        #expect(decoded.isPinned == true)
        #expect(decoded.projectPath == testPath)
        #expect(decoded.paneTitle == "Shell")
        #expect(decoded.filePath == nil)
    }

    @Test("TerminalTabSnapshot Codable round-trip for editor")
    func editorTabSnapshotRoundTrip() throws {
        let snapshot = TerminalTabSnapshot(
            kind: .editor,
            customTitle: nil,
            colorID: nil,
            isPinned: false,
            projectPath: testPath,
            paneTitle: nil,
            filePath: "/tmp/test/file.swift"
        )
        let data = try JSONEncoder().encode(snapshot)
        let decoded = try JSONDecoder().decode(TerminalTabSnapshot.self, from: data)

        #expect(decoded.kind == .editor)
        #expect(decoded.filePath == "/tmp/test/file.swift")
        #expect(decoded.paneTitle == "Terminal")
    }

    @Test("TerminalTabSnapshot decoding with missing kind defaults to terminal")
    func terminalTabSnapshotMissingKind() throws {
        let json = """
        {
            "customTitle": null,
            "isPinned": false,
            "projectPath": "/tmp/test",
            "paneTitle": "Shell"
        }
        """
        let data = Data(json.utf8)
        let decoded = try JSONDecoder().decode(TerminalTabSnapshot.self, from: data)
        #expect(decoded.kind == .terminal)
    }

    @Test("TabAreaSnapshot Codable round-trip")
    func tabAreaSnapshotRoundTrip() throws {
        let areaID = UUID()
        let snapshot = TabAreaSnapshot(
            id: areaID,
            projectPath: testPath,
            tabs: [
                TerminalTabSnapshot(kind: .terminal, customTitle: nil, colorID: nil, isPinned: false, projectPath: testPath, paneTitle: "Shell"),
                TerminalTabSnapshot(kind: .vcs, customTitle: nil, colorID: nil, isPinned: false, projectPath: testPath, paneTitle: "Git Diff"),
            ],
            activeTabIndex: 1
        )
        let data = try JSONEncoder().encode(snapshot)
        let decoded = try JSONDecoder().decode(TabAreaSnapshot.self, from: data)

        #expect(decoded.id == areaID)
        #expect(decoded.projectPath == testPath)
        #expect(decoded.tabs.count == 2)
        #expect(decoded.activeTabIndex == 1)
    }

    @Test("SplitNodeSnapshot.tabArea Codable round-trip")
    func splitNodeTabAreaRoundTrip() throws {
        let areaSnapshot = TabAreaSnapshot(
            id: UUID(),
            projectPath: testPath,
            tabs: [TerminalTabSnapshot(kind: .terminal, customTitle: nil, colorID: nil, isPinned: false, projectPath: testPath, paneTitle: "Shell")],
            activeTabIndex: 0
        )
        let snapshot = SplitNodeSnapshot.tabArea(areaSnapshot)
        let data = try JSONEncoder().encode(snapshot)
        let decoded = try JSONDecoder().decode(SplitNodeSnapshot.self, from: data)

        if case let .tabArea(decodedArea) = decoded {
            #expect(decodedArea.id == areaSnapshot.id)
        } else {
            Issue.record("Expected tabArea snapshot")
        }
    }

    @Test("SplitNodeSnapshot.split Codable round-trip")
    func splitNodeSplitRoundTrip() throws {
        let area1 = TabAreaSnapshot(
            id: UUID(), projectPath: testPath,
            tabs: [TerminalTabSnapshot(kind: .terminal, customTitle: nil, colorID: nil, isPinned: false, projectPath: testPath, paneTitle: "Shell")],
            activeTabIndex: 0
        )
        let area2 = TabAreaSnapshot(
            id: UUID(), projectPath: testPath,
            tabs: [TerminalTabSnapshot(kind: .vcs, customTitle: nil, colorID: nil, isPinned: false, projectPath: testPath, paneTitle: "VCS")],
            activeTabIndex: 0
        )
        let branchSnapshot = SplitBranchSnapshot(
            direction: .horizontal,
            ratio: 0.6,
            first: .tabArea(area1),
            second: .tabArea(area2)
        )
        let snapshot = SplitNodeSnapshot.split(branchSnapshot)
        let data = try JSONEncoder().encode(snapshot)
        let decoded = try JSONDecoder().decode(SplitNodeSnapshot.self, from: data)

        if case let .split(decodedBranch) = decoded {
            #expect(decodedBranch.direction == .horizontal)
            #expect(abs(decodedBranch.ratio - 0.6) < 0.001)
        } else {
            Issue.record("Expected split snapshot")
        }
    }

    @Test("WorkspaceSnapshot Codable round-trip")
    func workspaceSnapshotRoundTrip() throws {
        let projectID = UUID()
        let worktreeID = UUID()
        let focusedAreaID = UUID()
        let snapshot = WorkspaceSnapshot(
            projectID: projectID,
            worktreeID: worktreeID,
            worktreePath: testPath,
            focusedAreaID: focusedAreaID,
            root: .tabArea(TabAreaSnapshot(
                id: focusedAreaID, projectPath: testPath,
                tabs: [TerminalTabSnapshot(kind: .terminal, customTitle: nil, colorID: nil, isPinned: false, projectPath: testPath, paneTitle: "Shell")],
                activeTabIndex: 0
            ))
        )
        let data = try JSONEncoder().encode(snapshot)
        let decoded = try JSONDecoder().decode(WorkspaceSnapshot.self, from: data)

        #expect(decoded.projectID == projectID)
        #expect(decoded.worktreeID == worktreeID)
        #expect(decoded.worktreePath == testPath)
        #expect(decoded.focusedAreaID == focusedAreaID)
    }

    @Test("WorkspaceRestorer.snapshotAll produces correct structure")
    func snapshotAll() {
        let projectID = UUID()
        let worktreeID = UUID()
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = TabArea(projectPath: testPath)
        let root = SplitNode.tabArea(area)
        let workspaceRoots: [WorktreeKey: SplitNode] = [key: root]
        let focusedAreaID: [WorktreeKey: UUID] = [key: area.id]

        let snapshots = WorkspaceRestorer.snapshotAll(
            workspaceRoots: workspaceRoots,
            focusedAreaID: focusedAreaID
        )

        #expect(snapshots.count == 1)
        #expect(snapshots[0].projectID == projectID)
        #expect(snapshots[0].worktreeID == worktreeID)
        #expect(snapshots[0].focusedAreaID == area.id)
    }

    @Test("WorkspaceRestorer.restoreAll rebuilds tree from snapshots")
    func restoreAll() {
        let project = Project(name: "Test", path: testPath)
        let worktree = Worktree(name: "main", path: testPath, isPrimary: true)
        let areaID = UUID()

        let snapshot = WorkspaceSnapshot(
            projectID: project.id,
            worktreeID: worktree.id,
            worktreePath: testPath,
            focusedAreaID: areaID,
            root: .tabArea(TabAreaSnapshot(
                id: areaID, projectPath: testPath,
                tabs: [TerminalTabSnapshot(kind: .terminal, customTitle: nil, colorID: nil, isPinned: false, projectPath: testPath, paneTitle: "Shell")],
                activeTabIndex: 0
            ))
        )

        let results = WorkspaceRestorer.restoreAll(
            from: [snapshot],
            projects: [project],
            worktrees: [project.id: [worktree]]
        )

        #expect(results.count == 1)
        #expect(results[0].key.projectID == project.id)
        #expect(results[0].key.worktreeID == worktree.id)
        #expect(results[0].root.allAreas().count == 1)
        #expect(results[0].focusedAreaID == areaID)
    }

    @Test("WorkspaceRestorer.restoreAll skips missing projects")
    func restoreAllSkipsMissingProjects() {
        let snapshot = WorkspaceSnapshot(
            projectID: UUID(),
            worktreeID: UUID(),
            worktreePath: testPath,
            focusedAreaID: UUID(),
            root: .tabArea(TabAreaSnapshot(
                id: UUID(), projectPath: testPath,
                tabs: [TerminalTabSnapshot(kind: .terminal, customTitle: nil, colorID: nil, isPinned: false, projectPath: testPath, paneTitle: "Shell")],
                activeTabIndex: 0
            ))
        )

        let results = WorkspaceRestorer.restoreAll(
            from: [snapshot],
            projects: [],
            worktrees: [:]
        )

        #expect(results.isEmpty)
    }

    @Test("WorkspaceRestorer.restoreAll falls back to primary worktree")
    func restoreAllFallbackToPrimary() {
        let project = Project(name: "Test", path: testPath)
        let primaryWorktree = Worktree(name: "main", path: testPath, isPrimary: true)

        let snapshot = WorkspaceSnapshot(
            projectID: project.id,
            worktreeID: UUID(),
            worktreePath: testPath,
            focusedAreaID: nil,
            root: .tabArea(TabAreaSnapshot(
                id: UUID(), projectPath: testPath,
                tabs: [TerminalTabSnapshot(kind: .terminal, customTitle: nil, colorID: nil, isPinned: false, projectPath: testPath, paneTitle: "Shell")],
                activeTabIndex: 0
            ))
        )

        let results = WorkspaceRestorer.restoreAll(
            from: [snapshot],
            projects: [project],
            worktrees: [project.id: [primaryWorktree]]
        )

        #expect(results.count == 1)
        #expect(results[0].key.worktreeID == primaryWorktree.id)
    }

    @Test("TabArea snapshot and restore round-trip")
    func tabAreaRoundTrip() {
        let area = TabArea(projectPath: testPath)
        area.createTab()
        area.togglePin(area.tabs[0].id)

        let snapshot = area.snapshot()
        let restored = TabArea(restoring: snapshot)

        #expect(restored.id == area.id)
        #expect(restored.projectPath == area.projectPath)
        #expect(restored.tabs.count == area.tabs.count)
        #expect(restored.tabs[0].isPinned == true)
    }
}
