import AppKit
import os
import SwiftUI

private let logger = Logger(subsystem: "app.muxy", category: "WorktreeRefreshHelper")

@MainActor
enum WorktreeRefreshHelper {
    static func refresh(
        project: Project,
        appState: AppState,
        worktreeStore: WorktreeStore,
        isRefreshing: Binding<Bool>? = nil,
        presentErrors: Bool = true
    ) async {
        if isRefreshing?.wrappedValue == true { return }
        let previous = worktreeStore.list(for: project.id)
        isRefreshing?.wrappedValue = true
        defer { isRefreshing?.wrappedValue = false }

        do {
            let refreshed = try await worktreeStore.refreshFromGit(project: project)
            let refreshedIDs = Set(refreshed.map(\.id))
            let replacement = appState.activeWorktreeID[project.id].flatMap { activeID in
                refreshed.first { $0.id == activeID }
            } ?? refreshed.first(where: \.isPrimary) ?? refreshed.first

            for worktree in previous where !refreshedIDs.contains(worktree.id) {
                appState.removeWorktree(projectID: project.id, worktree: worktree, replacement: replacement)
            }
        } catch {
            guard presentErrors else {
                logger
                    .error("Worktree refresh failed for \(project.path, privacy: .public): \(error.localizedDescription, privacy: .public)")
                return
            }
            presentError(error.localizedDescription)
        }
    }

    static func presentError(_ message: String) {
        guard let window = NSApp.keyWindow ?? NSApp.mainWindow,
              window.attachedSheet == nil
        else { return }

        let alert = NSAlert()
        alert.messageText = "Could Not Refresh Worktrees"
        alert.informativeText = message
        alert.alertStyle = .warning
        alert.icon = NSApp.applicationIconImage
        alert.addButton(withTitle: "OK")
        alert.buttons[0].keyEquivalent = "\r"
        alert.beginSheetModal(for: window)
    }
}
