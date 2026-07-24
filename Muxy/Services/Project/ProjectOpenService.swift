import AppKit

@MainActor
enum ProjectOpenService {
    enum RestoreError: LocalizedError, Equatable {
        case unavailable
        case missingDirectory(String)
        case notDirectory(String)
        case worktreeUnavailable
        case remoteWorkspaceActive
        case persistenceFailed

        var errorDescription: String? {
            switch self {
            case .unavailable:
                "This project is no longer available in Recently Removed."
            case let .missingDirectory(path):
                "The project folder no longer exists at \(path)."
            case let .notDirectory(path):
                "The project path is no longer a folder: \(path)."
            case .worktreeUnavailable:
                "Muxy could not prepare the project workspace."
            case .remoteWorkspaceActive:
                "Switch to a local workspace before restoring this project."
            case .persistenceFailed:
                "Muxy could not save the restored project."
            }
        }
    }

    static func openProject(
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore,
        projectGroupStore: ProjectGroupStore
    ) {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.message = "Select a project folder"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        confirmProjectPath(
            url.path(percentEncoded: false),
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )
    }

    static func openProjectViaPicker(
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore,
        projectGroupStore: ProjectGroupStore,
        preferences: ProjectPickerPreferences = ProjectPickerPreferences(),
        notificationCenter: NotificationCenter = .default,
        openWithFinder: (() -> Void)? = nil
    ) {
        let finder = ProjectOpenFinderPresentationAdapter {
            if let openWithFinder {
                openWithFinder()
            } else {
                openProject(
                    appState: appState,
                    projectStore: projectStore,
                    worktreeStore: worktreeStore,
                    projectGroupStore: projectGroupStore
                )
            }
        }
        presentOpenProject(
            preferences: preferences,
            customPicker: ProjectOpenCustomPickerPresentationAdapter(notificationCenter: notificationCenter),
            finder: finder
        )
    }

    @discardableResult
    static func restoreRecentlyRemovedProject(
        id: UUID,
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore,
        projectGroupStore: ProjectGroupStore,
        fileSystem: any ProjectPathConfirmationFileSystem = FileManagerProjectPathConfirmationFileSystem()
    ) throws -> Project {
        guard !projectGroupStore.isRemoteWorkspaceActive else {
            throw RestoreError.remoteWorkspaceActive
        }
        guard let entry = projectStore.recentlyRemovedProjects.first(where: { $0.id == id }) else {
            throw RestoreError.unavailable
        }
        let project = entry.project
        switch fileSystem.directoryState(atPath: project.path) {
        case .missing:
            throw RestoreError.missingDirectory(project.path)
        case .notDirectory:
            throw RestoreError.notDirectory(project.path)
        case .directory:
            break
        }

        let previousWorktrees = worktreeStore.list(for: project.id)
        worktreeStore.ensurePrimary(for: project)
        guard let primary = worktreeStore.primary(for: project.id) else {
            worktreeStore.restoreProjectWorktrees(previousWorktrees, for: project.id)
            throw RestoreError.worktreeUnavailable
        }
        guard let restoredProject = projectStore.restoreRecentlyRemovedProject(id: id) else {
            worktreeStore.restoreProjectWorktrees(previousWorktrees, for: project.id)
            throw RestoreError.persistenceFailed
        }
        projectGroupStore.addProjectToActiveGroup(projectID: restoredProject.id)
        appState.selectProject(restoredProject, worktree: primary)
        return restoredProject
    }

    static func presentOpenProject(
        preferences: ProjectPickerPreferences = ProjectPickerPreferences(),
        notificationCenter: NotificationCenter = .default,
        openWithFinder: @escaping () -> Void
    ) {
        presentOpenProject(
            preferences: preferences,
            customPicker: ProjectOpenCustomPickerPresentationAdapter(notificationCenter: notificationCenter),
            finder: ProjectOpenFinderPresentationAdapter(presentFinder: openWithFinder)
        )
    }

    static func presentOpenProject(
        preferences: ProjectPickerPreferences = ProjectPickerPreferences(),
        customPicker: ProjectOpenCustomPickerPresentationAdapter = ProjectOpenCustomPickerPresentationAdapter(),
        finder: ProjectOpenFinderPresentationAdapter
    ) {
        ProjectOpenPresentationRouter(
            preferences: preferences,
            customPicker: customPicker,
            finder: finder
        )
        .present()
    }

    @discardableResult
    static func confirmProjectPath(
        _ path: String,
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore,
        projectGroupStore: ProjectGroupStore,
        createIfMissing: Bool = false
    ) -> Bool {
        confirmProjectPathResult(
            path,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore,
            createIfMissing: createIfMissing
        ).didConfirm
    }

    @discardableResult
    static func confirmProjectPathResult(
        _ path: String,
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore,
        projectGroupStore: ProjectGroupStore,
        createIfMissing: Bool = false
    ) -> ProjectOpenConfirmationResult {
        ProjectPathConfirmationService(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )
        .confirm(path: path, createIfMissing: createIfMissing)
    }
}
