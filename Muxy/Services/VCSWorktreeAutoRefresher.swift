import Foundation

@MainActor
final class VCSWorktreeAutoRefresher {
    private let appState: AppState
    private let projectStore: ProjectStore
    private let worktreeStore: WorktreeStore
    nonisolated(unsafe) private var observers: [NSObjectProtocol] = []
    private var inFlight: Set<UUID> = []
    private var pending: Set<UUID> = []

    init(appState: AppState, projectStore: ProjectStore, worktreeStore: WorktreeStore) {
        self.appState = appState
        self.projectStore = projectStore
        self.worktreeStore = worktreeStore
        observe(.vcsDidRefresh)
        observe(.vcsRepoDidChange)
    }

    deinit {
        for observer in observers {
            NotificationCenter.default.removeObserver(observer)
        }
    }

    private func observe(_ name: Notification.Name) {
        let token = NotificationCenter.default.addObserver(
            forName: name,
            object: nil,
            queue: .main
        ) { [weak self] notification in
            guard let path = notification.userInfo?["repoPath"] as? String else { return }
            MainActor.assumeIsolated {
                self?.handleRefresh(repoPath: path)
            }
        }
        observers.append(token)
    }

    private func handleRefresh(repoPath: String) {
        guard let projectID = worktreeStore.projectID(forWorktreePath: repoPath) else { return }
        guard let project = projectStore.projects.first(where: { $0.id == projectID }) else { return }
        guard !inFlight.contains(projectID) else {
            pending.insert(projectID)
            return
        }
        runRefresh(project: project)
    }

    private func runRefresh(project: Project) {
        inFlight.insert(project.id)
        Task { [appState, worktreeStore, projectStore] in
            await WorktreeRefreshHelper.refresh(
                project: project,
                appState: appState,
                worktreeStore: worktreeStore,
                isRefreshing: nil,
                presentErrors: false
            )
            inFlight.remove(project.id)
            guard pending.remove(project.id) != nil else { return }
            guard let updated = projectStore.projects.first(where: { $0.id == project.id }) else { return }
            runRefresh(project: updated)
        }
    }
}
