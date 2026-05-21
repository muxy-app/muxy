import Foundation
import os

private let terminalSessionLogger = Logger(subsystem: "app.muxy", category: "TerminalSessionStore")

@MainActor
protocol TerminalSessionStoring: AnyObject {
    var sessionsByPaneID: [UUID: TerminalSessionSnapshot] { get }
    func save(workspaceRoots: [WorktreeKey: SplitNode])
    func recordClosedTerminalTab(_ snapshot: ClosedTerminalTabSnapshot, workspaceRoots: [WorktreeKey: SplitNode])
    func popLastClosedTerminalTab(projectID: UUID, worktreeID: UUID) -> ClosedTerminalTabSnapshot?
    func nextClosedSequence() -> Int64
}

@MainActor
final class TerminalSessionStore {
    static let shared = TerminalSessionStore()

    private let store: CodableFileStore<TerminalSessionFile>
    private(set) var sessionsByPaneID: [UUID: TerminalSessionSnapshot] = [:]
    private(set) var closedTerminalTabs: [ClosedTerminalTabSnapshot] = []

    private static let maxClosedTerminalTabs = 50

    private init(fileURL: URL = MuxyFileStorage.fileURL(filename: "terminal-sessions.json")) {
        store = CodableFileStore(fileURL: fileURL, options: .pretty)
        load()
    }

    func load() {
        do {
            guard let file = try store.load() else {
                sessionsByPaneID = [:]
                closedTerminalTabs = []
                return
            }
            sessionsByPaneID = Dictionary(uniqueKeysWithValues: file.sessions.map { ($0.paneID, $0) })
            closedTerminalTabs = file.closedTerminalTabs
        } catch {
            terminalSessionLogger.error("Failed to load terminal sessions: \(error)")
            sessionsByPaneID = [:]
            closedTerminalTabs = []
        }
    }

    func session(for paneID: UUID) -> TerminalSessionSnapshot? {
        sessionsByPaneID[paneID]
    }

    func save(workspaceRoots: [WorktreeKey: SplitNode]) {
        guard SessionRestorePreferences.isEnabled else { return }
        let snapshots = Self.retainedSnapshots(
            buildSnapshots(workspaceRoots: workspaceRoots),
            maxPerWorktree: SessionRestorePreferences.maxSnapshots
        )
        saveFile(sessions: snapshots)
    }

    static func retainedSnapshots(
        _ snapshots: [TerminalSessionSnapshot],
        maxPerWorktree: Int
    ) -> [TerminalSessionSnapshot] {
        guard maxPerWorktree > 0 else { return [] }
        return Dictionary(grouping: snapshots) { snapshot in
            WorktreeKey(projectID: snapshot.projectID, worktreeID: snapshot.worktreeID)
        }
        .values
        .flatMap { group in
            group
                .sorted { $0.capturedAt > $1.capturedAt }
                .prefix(maxPerWorktree)
        }
        .sorted { $0.capturedAt > $1.capturedAt }
    }

    func recordClosedTerminalTab(_ snapshot: ClosedTerminalTabSnapshot) {
        insertClosedTerminalTab(snapshot)
        saveFile(sessions: Array(sessionsByPaneID.values))
    }

    func recordClosedTerminalTab(_ snapshot: ClosedTerminalTabSnapshot, workspaceRoots: [WorktreeKey: SplitNode]) {
        insertClosedTerminalTab(snapshot)
        saveFile(sessions: sessionsForCurrentPreferences(workspaceRoots: workspaceRoots))
    }

    private func insertClosedTerminalTab(_ snapshot: ClosedTerminalTabSnapshot) {
        closedTerminalTabs.removeAll { existing in
            existing.projectID == snapshot.projectID
                && existing.worktreeID == snapshot.worktreeID
                && existing.areaID == snapshot.areaID
                && existing.title == snapshot.title
                && existing.workingDirectory == snapshot.workingDirectory
                && existing.commandToRestore == snapshot.commandToRestore
        }
        closedTerminalTabs.append(snapshot)
        closedTerminalTabs = Array(
            closedTerminalTabs
                .sorted { $0.closedSequence > $1.closedSequence }
                .prefix(Self.maxClosedTerminalTabs)
        )
    }

    func popLastClosedTerminalTab(projectID: UUID, worktreeID: UUID) -> ClosedTerminalTabSnapshot? {
        guard let index = closedTerminalTabs
            .enumerated()
            .filter({ $0.element.projectID == projectID && $0.element.worktreeID == worktreeID })
            .max(by: { $0.element.closedSequence < $1.element.closedSequence })?
            .offset
        else { return nil }
        let snapshot = closedTerminalTabs.remove(at: index)
        saveFile(sessions: Array(sessionsByPaneID.values))
        return snapshot
    }

    func nextClosedSequence() -> Int64 {
        (closedTerminalTabs.map(\.closedSequence).max() ?? 0) + 1
    }

    private func buildSnapshots(workspaceRoots: [WorktreeKey: SplitNode]) -> [TerminalSessionSnapshot] {
        var snapshots: [TerminalSessionSnapshot] = []
        for (key, root) in workspaceRoots {
            for area in root.allAreas() {
                for tab in area.tabs {
                    guard let pane = tab.content.pane else { continue }
                    let view = TerminalViewRegistry.shared.existingView(for: pane.id)
                    let isRunning = view.map { $0.needsConfirmQuit() } ?? false
                    let hasRestoredCommand = pane.activeRestoredCommand != nil
                    let trackedCommand = TerminalCommandTracker.shared.lastSubmittedCommand(for: pane.id)
                        ?? pane.activeRestoredCommand
                    let activity: TerminalSessionSnapshot.Activity = isRunning || hasRestoredCommand ? .running : .idle
                    let cwd = pane.currentWorkingDirectory ?? pane.projectPath
                    snapshots.append(TerminalSessionSnapshot(
                        id: UUID(),
                        projectID: key.projectID,
                        worktreeID: key.worktreeID,
                        paneID: pane.id,
                        tabID: tab.id,
                        areaID: area.id,
                        projectPath: pane.projectPath,
                        title: pane.title,
                        workingDirectory: cwd,
                        startupCommand: pane.startupCommand,
                        lastSubmittedCommand: trackedCommand,
                        activity: activity,
                        capturedAt: Date()
                    ))
                }
            }
        }
        return snapshots
    }

    private func sessionsForCurrentPreferences(workspaceRoots: [WorktreeKey: SplitNode]) -> [TerminalSessionSnapshot] {
        guard SessionRestorePreferences.isEnabled else { return Array(sessionsByPaneID.values) }
        return Self.retainedSnapshots(
            buildSnapshots(workspaceRoots: workspaceRoots),
            maxPerWorktree: SessionRestorePreferences.maxSnapshots
        )
    }

    private func saveFile(sessions: [TerminalSessionSnapshot]) {
        do {
            try store.save(TerminalSessionFile(
                schemaVersion: TerminalSessionFile.currentSchemaVersion,
                sessions: sessions,
                closedTerminalTabs: closedTerminalTabs
            ))
            sessionsByPaneID = Dictionary(uniqueKeysWithValues: sessions.map { ($0.paneID, $0) })
        } catch {
            terminalSessionLogger.error("Failed to save terminal sessions: \(error)")
        }
    }
}

extension TerminalSessionStore: TerminalSessionStoring {}
