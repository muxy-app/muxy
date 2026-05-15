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
        confirmProjectPath(
            url.path(percentEncoded: false),
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore
        )
    }

    @discardableResult
    static func confirmProjectPath(
        _ path: String,
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore,
        createIfMissing: Bool = false
    ) -> Bool {
        let standardizedPath = URL(fileURLWithPath: path).standardizedFileURL.path
        var isDirectory: ObjCBool = false
        if !FileManager.default.fileExists(atPath: standardizedPath, isDirectory: &isDirectory) {
            guard createIfMissing else { return false }
            do {
                try FileManager.default.createDirectory(
                    at: URL(fileURLWithPath: standardizedPath),
                    withIntermediateDirectories: true
                )
                isDirectory = true
            } catch {
                return false
            }
        }
        guard isDirectory.boolValue else { return false }

        if let existing = projectStore.projects.first(where: { $0.path == standardizedPath }),
           let primary = worktreeStore.primary(for: existing.id)
        {
            appState.selectProject(existing, worktree: primary)
            return true
        }

        let url = URL(fileURLWithPath: standardizedPath)
        let project = Project(
            name: url.lastPathComponent,
            path: standardizedPath,
            sortOrder: projectStore.projects.count
        )
        projectStore.add(project)
        worktreeStore.ensurePrimary(for: project)
        guard let primary = worktreeStore.primary(for: project.id) else { return false }
        appState.selectProject(project, worktree: primary)
        return true
    }
}
