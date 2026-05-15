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
        switch preferences.mode {
        case .custom:
            notificationCenter.post(name: .openProjectPicker, object: nil)
        case .finder:
            if let openWithFinder {
                openWithFinder()
            } else {
                openProject(appState: appState, projectStore: projectStore, worktreeStore: worktreeStore)
            }
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
        let standardizedPath = URL(fileURLWithPath: path).standardizedFileURL.path
        var isDirectory: ObjCBool = false
        if !FileManager.default.fileExists(atPath: standardizedPath, isDirectory: &isDirectory) {
            guard createIfMissing else { return .missingDirectory }
            do {
                try FileManager.default.createDirectory(
                    at: URL(fileURLWithPath: standardizedPath),
                    withIntermediateDirectories: true
                )
                isDirectory = true
            } catch {
                return .createFailed
            }
        }
        guard isDirectory.boolValue else { return .notDirectory }

        if let existing = projectStore.projects.first(where: { $0.path == standardizedPath }) {
            worktreeStore.ensurePrimary(for: existing)
            guard let primary = worktreeStore.primary(for: existing.id) else { return .failed }
            appState.selectProject(existing, worktree: primary)
            return .success
        }

        let url = URL(fileURLWithPath: standardizedPath)
        let project = Project(
            name: url.lastPathComponent,
            path: standardizedPath,
            sortOrder: projectStore.projects.count
        )
        projectStore.add(project)
        worktreeStore.ensurePrimary(for: project)
        guard let primary = worktreeStore.primary(for: project.id) else { return .failed }
        appState.selectProject(project, worktree: primary)
        return .success
    }
}
