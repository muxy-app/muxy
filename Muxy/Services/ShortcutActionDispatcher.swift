import Foundation

@MainActor
struct ShortcutActionDispatcher {
    let appState: AppState
    let projectStore: ProjectStore
    let worktreeStore: WorktreeStore
    let ghostty: GhosttyService
    let notificationCenter: NotificationCenter

    init(
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore,
        ghostty: GhosttyService,
        notificationCenter: NotificationCenter = .default
    ) {
        self.appState = appState
        self.projectStore = projectStore
        self.worktreeStore = worktreeStore
        self.ghostty = ghostty
        self.notificationCenter = notificationCenter
    }

    func perform(_ action: ShortcutAction, activeProject: Project?, openVCS: (Project) -> Void) -> Bool {
        if let index = action.tabSelectionIndex {
            guard let projectID = appState.activeProjectID else { return false }
            appState.selectTabByIndex(index, projectID: projectID)
            return true
        }

        if let index = action.projectSelectionIndex {
            appState.selectProjectByIndex(index, projects: projectStore.projects, worktrees: worktreeStore.worktrees)
            return true
        }

        switch action {
        case .newTab:
            guard let projectID = appState.activeProjectID else { return false }
            if appState.workspaceRoot(for: projectID) == nil {
                guard let worktree = resolveActiveWorktree(for: projectID) else { return false }
                appState.selectWorktree(projectID: projectID, worktree: worktree)
                return true
            }
            appState.createTab(projectID: projectID)
            return true
        case .closeTab:
            guard let projectID = appState.activeProjectID,
                  let area = appState.focusedArea(for: projectID),
                  let tabID = area.activeTabID
            else { return false }
            appState.closeTab(tabID, projectID: projectID)
            return true
        case .renameTab:
            notificationCenter.post(name: .renameActiveTab, object: nil)
            return true
        case .pinUnpinTab:
            guard let projectID = appState.activeProjectID else { return false }
            appState.togglePinActiveTab(projectID: projectID)
            return true
        case .splitRight:
            guard let projectID = appState.activeProjectID else { return false }
            appState.splitFocusedArea(direction: .horizontal, projectID: projectID)
            return true
        case .splitDown:
            guard let projectID = appState.activeProjectID else { return false }
            appState.splitFocusedArea(direction: .vertical, projectID: projectID)
            return true
        case .closePane:
            guard let projectID = appState.activeProjectID,
                  let areaID = appState.focusedAreaID(for: projectID)
            else { return false }
            appState.closeArea(areaID, projectID: projectID)
            return true
        case .focusPaneLeft:
            guard let projectID = appState.activeProjectID else { return false }
            appState.focusPaneLeft(projectID: projectID)
            return true
        case .focusPaneRight:
            guard let projectID = appState.activeProjectID else { return false }
            appState.focusPaneRight(projectID: projectID)
            return true
        case .focusPaneUp:
            guard let projectID = appState.activeProjectID else { return false }
            appState.focusPaneUp(projectID: projectID)
            return true
        case .focusPaneDown:
            guard let projectID = appState.activeProjectID else { return false }
            appState.focusPaneDown(projectID: projectID)
            return true
        case .nextTab:
            guard let projectID = appState.activeProjectID else { return false }
            appState.selectNextTab(projectID: projectID)
            return true
        case .previousTab:
            guard let projectID = appState.activeProjectID else { return false }
            appState.selectPreviousTab(projectID: projectID)
            return true
        case .toggleThemePicker:
            notificationCenter.post(name: .toggleThemePicker, object: nil)
            return true
        case .newProject:
            return false
        case .openProject:
            ProjectOpenService.openProject(
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore
            )
            return true
        case .reloadConfig:
            ghostty.reloadConfig()
            return true
        case .nextProject:
            appState.selectNextProject(projects: projectStore.projects, worktrees: worktreeStore.worktrees)
            return true
        case .previousProject:
            appState.selectPreviousProject(projects: projectStore.projects, worktrees: worktreeStore.worktrees)
            return true
        case .findInTerminal:
            notificationCenter.post(name: .findInTerminal, object: nil)
            return true
        case .openVCSTab:
            guard let activeProject else { return false }
            openVCS(activeProject)
            return true
        case .quickOpen:
            notificationCenter.post(name: .quickOpen, object: nil)
            return true
        case .switchWorktree:
            notificationCenter.post(name: .switchWorktree, object: nil)
            return true
        case .saveFile:
            notificationCenter.post(name: .saveActiveEditor, object: nil)
            return true
        case .toggleSidebar:
            notificationCenter.post(name: .toggleSidebar, object: nil)
            return true
        case .toggleFileTree:
            notificationCenter.post(name: .toggleFileTree, object: nil)
            return true
        case .navigateBack:
            guard appState.navigation.canGoBack else { return false }
            appState.goBack()
            return true
        case .navigateForward:
            guard appState.navigation.canGoForward else { return false }
            appState.goForward()
            return true
        case .selectTab1,
             .selectTab2,
             .selectTab3,
             .selectTab4,
             .selectTab5,
             .selectTab6,
             .selectTab7,
             .selectTab8,
             .selectTab9,
             .selectProject1,
             .selectProject2,
             .selectProject3,
             .selectProject4,
             .selectProject5,
             .selectProject6,
             .selectProject7,
             .selectProject8,
             .selectProject9:
            return false
        }
    }

    private func resolveActiveWorktree(for projectID: UUID) -> Worktree? {
        worktreeStore.preferred(for: projectID, matching: appState.activeWorktreeID[projectID])
    }
}
