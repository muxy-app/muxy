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
            sortOrder: 0
        )
        projectStore.insert(project, afterProjectWithID: appState.activeProjectID)
        worktreeStore.ensurePrimary(for: project)
        guard let primary = worktreeStore.primary(for: project.id) else { return }
        appState.selectProject(project, worktree: primary)
    }

    static func duplicateActiveProject(
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore
    ) -> Bool {
        guard let activeProjectID = appState.activeProjectID,
              let activeProject = projectStore.projects.first(where: { $0.id == activeProjectID })
        else { return false }
        let project = Project(
            name: activeProject.name,
            path: activeProject.path,
            sortOrder: 0
        )
        projectStore.insert(project, afterProjectWithID: activeProjectID)
        worktreeStore.ensurePrimary(for: project)
        guard let primary = worktreeStore.primary(for: project.id) else { return false }
        appState.selectProject(project, worktree: primary)
        return true
    }
}
