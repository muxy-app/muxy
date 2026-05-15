import AppKit

enum ProjectOpenConfirmationResult: Equatable {
    case success
    case missingDirectory
    case notDirectory
    case createFailed
    case failed

    var didConfirm: Bool {
        self == .success
    }
}

@MainActor
enum ProjectOpenService {
    static func openProject(
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore
    ) {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.message = "Select a project folder"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        let project = Project(
            name: url.lastPathComponent,
            path: url.path(percentEncoded: false),
            sortOrder: projectStore.projects.count
        )
        projectStore.add(project)
        worktreeStore.ensurePrimary(for: project)
        guard let primary = worktreeStore.primary(for: project.id) else { return }
        appState.selectProject(project, worktree: primary)
    }

    static func openProjectViaPicker(
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore,
        preferences: ProjectPickerPreferences = ProjectPickerPreferences(),
        notificationCenter: NotificationCenter = .default,
        openWithFinder: (() -> Void)? = nil
    ) {
        presentOpenProject(preferences: preferences, notificationCenter: notificationCenter) {
            if let openWithFinder {
                openWithFinder()
            } else {
                openProject(appState: appState, projectStore: projectStore, worktreeStore: worktreeStore)
            }
        }
    }

    static func presentOpenProject(
        preferences: ProjectPickerPreferences = ProjectPickerPreferences(),
        notificationCenter: NotificationCenter = .default,
        openWithFinder: () -> Void
    ) {
        switch ProjectOpenPresentationRouter(preferences: preferences).route() {
        case .customPicker:
            notificationCenter.post(name: .openProjectPicker, object: nil)
        case .finder:
            openWithFinder()
        }
    }

    @discardableResult
    static func confirmProjectPath(
        _ path: String,
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore,
        createIfMissing: Bool = false
    ) -> Bool {
        confirmProjectPathResult(
            path,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            createIfMissing: createIfMissing
        ).didConfirm
    }

    @discardableResult
    static func confirmProjectPathResult(
        _ path: String,
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore,
        createIfMissing: Bool = false
    ) -> ProjectOpenConfirmationResult {
        ProjectPathConfirmationService(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore
        )
        .confirm(path: path, createIfMissing: createIfMissing)
    }
}
