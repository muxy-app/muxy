import AppKit
import SwiftUI

@MainActor
enum WorktreeRefreshHelper {
    static func refresh(
        project: Project,
        worktreeStore: WorktreeStore,
        isRefreshing: Binding<Bool>
    ) async {
        guard !isRefreshing.wrappedValue else { return }
        isRefreshing.wrappedValue = true
        defer { isRefreshing.wrappedValue = false }

        do {
            _ = try await worktreeStore.refreshFromGit(project: project)
        } catch {
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
