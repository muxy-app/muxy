import Foundation

@MainActor
final class VCSWorktreeAutoRefresher {
    private let appState: AppState
    private let projectStore: ProjectStore
    private let worktreeStore: WorktreeStore
    private let projectGroupStore: ProjectGroupStore
    nonisolated(unsafe) private var observers: [NSObjectProtocol] = []
    private var inFlight: Set<UUID> = []
    private var pending: Set<UUID> = []
    private var watchers: [UUID: GitWorktreesWatcher] = [:]

    init(appState: AppState, projectStore: ProjectStore, worktreeStore: WorktreeStore, projectGroupStore: ProjectGroupStore) {
        self.appState = appState
        self.projectStore = projectStore
        self.worktreeStore = worktreeStore
        self.projectGroupStore = projectGroupStore
        observe(.vcsDidRefresh)
        observe(.vcsRepoDidChange)
        syncWatchers()
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
        handleRefresh(projectID: projectID)
    }

    private func handleRefresh(projectID: UUID) {
        guard let project = projectStore.projects.first(where: { $0.id == projectID }) else { return }
        guard !inFlight.contains(projectID) else {
            pending.insert(projectID)
            return
        }
        runRefresh(project: project)
    }

    private func syncWatchers() {
        withObservationTracking {
            reconcileWatchers()
        } onChange: {
            Task { @MainActor [weak self] in
                self?.syncWatchers()
            }
        }
    }

    private func reconcileWatchers() {
        let projects = projectStore.projects
        let currentIDs = Set(projects.map(\.id))

        for projectID in Array(watchers.keys) where !currentIDs.contains(projectID) {
            watchers.removeValue(forKey: projectID)
        }

        for project in projects where watchers[project.id] == nil {
            let projectID = project.id
            let watcher = GitWorktreesWatcher(repoPath: project.path) { [weak self] in
                Task { @MainActor [weak self] in
                    self?.handleRefresh(projectID: projectID)
                }
            }
            guard let watcher else { continue }
            watchers[projectID] = watcher
        }
    }

    private func runRefresh(project: Project) {
        inFlight.insert(project.id)
        Task { [appState, worktreeStore, projectStore, projectGroupStore] in
            await WorktreeRefreshHelper.refresh(
                project: project,
                appState: appState,
                worktreeStore: worktreeStore,
                projectGroupStore: projectGroupStore,
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
