import AppKit
import Foundation

@MainActor
struct ShortcutActionDispatcher {
    let appState: AppState
    let projectStore: ProjectStore
    let worktreeStore: WorktreeStore
    let projectGroupStore: ProjectGroupStore
    let ghostty: GhosttyService
    let notificationCenter: NotificationCenter

    init(
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore,
        projectGroupStore: ProjectGroupStore,
        ghostty: GhosttyService,
        notificationCenter: NotificationCenter = .default
    ) {
        self.appState = appState
        self.projectStore = projectStore
        self.worktreeStore = worktreeStore
        self.projectGroupStore = projectGroupStore
        self.ghostty = ghostty
        self.notificationCenter = notificationCenter
    }

    private var navigableProjects: [Project] {
        projectGroupStore.filteredProjects(from: projectStore.projects)
    }

    func perform(_ action: ShortcutAction, activeProject: Project?) -> Bool {
        if let index = action.tabSelectionIndex {
            guard let projectID = appState.activeProjectID else { return false }
            appState.selectTabByIndex(index, projectID: projectID)
            return true
        }

        if let index = action.projectSelectionIndex {
            appState.selectProjectByIndex(index, projects: navigableProjects, worktrees: worktreeStore.worktrees)
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
        case .reopenClosedTerminalTab:
            return appState.reopenLastClosedTerminalTab()
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
        case .cycleNextTabAcrossPanes:
            guard let projectID = appState.activeProjectID else { return false }
            appState.cycleNextTabAcrossPanes(projectID: projectID)
            return true
        case .cyclePreviousTabAcrossPanes:
            guard let projectID = appState.activeProjectID else { return false }
            appState.cyclePreviousTabAcrossPanes(projectID: projectID)
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
            ProjectOpenService.openProjectViaPicker(
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore,
                projectGroupStore: projectGroupStore
            )
            return true
        case .reloadConfig:
            ghostty.reloadConfig()
            return true
        case .refreshWorktrees:
            guard let activeProject else { return false }
            Task { @MainActor in
                await WorktreeRefreshHelper.refresh(
                    project: activeProject,
                    appState: appState,
                    worktreeStore: worktreeStore
                )
            }
            return true
        case .nextProject:
            appState.selectNextProject(projects: navigableProjects, worktrees: worktreeStore.worktrees)
            return true
        case .previousProject:
            appState.selectPreviousProject(projects: navigableProjects, worktrees: worktreeStore.worktrees)
            return true
        case .findInTerminal:
            notificationCenter.post(name: .findInTerminal, object: nil)
            return true
        case .toggleRichInput:
            notificationCenter.post(name: .toggleRichInput, object: nil)
            return true
        case .submitRichInput,
             .submitRichInputWithoutReturn:
            return false
        case .terminalOmnibox:
            postTerminalOmnibox(scope: .openTabs)
            return true
        case .terminalOmniboxProjects:
            postTerminalOmnibox(scope: .projects)
            return true
        case .terminalOmniboxWorktrees:
            postTerminalOmnibox(scope: .worktrees)
            return true
        case .terminalOmniboxCommands:
            postTerminalOmnibox(scope: .commandShortcuts)
            return true
        case .terminalOmniboxHistory:
            postTerminalOmnibox(scope: .history)
            return true
        case .toggleSidebar:
            notificationCenter.post(name: .toggleSidebar, object: nil)
            return true
        case .toggleExtensionConsole:
            notificationCenter.post(name: .toggleExtensionConsole, object: nil)
            return true
        case .navigateBack:
            guard appState.navigation.canGoBack else { return false }
            appState.goBack()
            return true
        case .navigateForward:
            guard appState.navigation.canGoForward else { return false }
            appState.goForward()
            return true
        case .toggleMaximizePane:
            guard let projectID = appState.activeProjectID,
                  let areaID = appState.focusedAreaID(for: projectID)
            else { return false }
            appState.toggleMaximize(areaID: areaID, for: projectID)
            return true
        case .toggleFullScreen:
            guard let window = AppDelegate.mainAppWindow() else { return false }
            window.toggleFullScreen(nil)
            return true
        case .toggleVoiceRecording,
             .selectTab1,
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

    private func postTerminalOmnibox(scope: TerminalOmniboxLaunchScope) {
        notificationCenter.post(
            name: .terminalOmnibox,
            object: nil,
            userInfo: ["launchScope": scope.rawValue]
        )
    }
}
