import AppKit

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
        let finder = ProjectOpenFinderPresentationAdapter {
            if let openWithFinder {
                openWithFinder()
            } else {
                openProject(appState: appState, projectStore: projectStore, worktreeStore: worktreeStore)
            }
        }
        presentOpenProject(
            preferences: preferences,
            customPicker: ProjectOpenCustomPickerPresentationAdapter(notificationCenter: notificationCenter),
            finder: finder
        )
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
