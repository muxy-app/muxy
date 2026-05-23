import Combine
import Foundation

@MainActor
final class BrowserDevServerAutoOpener {
    private weak var appState: AppState?
    private var cancellable: AnyCancellable?

    init(appState: AppState) {
        self.appState = appState
        cancellable = NotificationCenter.default
            .publisher(for: .devServerDetected)
            .sink { [weak self] notification in
                MainActor.assumeIsolated {
                    self?.handle(userInfo: notification.userInfo)
                }
            }
    }

    private func handle(userInfo: [AnyHashable: Any]?) {
        guard BrowserPreferences.autoOpenDevServer else { return }
        guard let appState else { return }
        guard let url = userInfo?[DevServerSnifferKeys.urlKey] as? String else { return }
        let paneID = userInfo?[DevServerSnifferKeys.paneIDKey] as? UUID

        guard let target = resolveTarget(appState: appState, paneID: paneID) else { return }

        ToastState.shared.show("Dev server detected: \(url) — opening browser tab")
        appState.dispatch(.createBrowserTabInWorktree(
            worktreeKey: target.worktreeKey,
            areaID: target.areaID,
            initialURL: url
        ))
    }

    private func resolveTarget(appState: AppState, paneID: UUID?) -> (worktreeKey: WorktreeKey, areaID: UUID)? {
        if let paneID, let location = appState.locatePaneTab(paneID: paneID) {
            return (location.worktreeKey, location.areaID)
        }
        guard let projectID = appState.activeProjectID,
              let worktreeKey = appState.activeWorktreeKey(for: projectID),
              let focusedAreaID = appState.focusedAreaID[worktreeKey]
        else { return nil }
        return (worktreeKey, focusedAreaID)
    }
}
