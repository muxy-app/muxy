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
        if projectGroupStore.isRemoteWorkspaceActive {
            let remoteHome = projectGroupStore.activeRemoteHomeProject.map { [$0] } ?? []
            return remoteHome + projectGroupStore.displayProjects(localProjects: projectStore.storedProjects)
        }
        let filtered = projectGroupStore.filteredProjects(from: projectStore.storedProjects)
        guard HomeProjectPreferences.isVisible else { return filtered }
        return [Project.home] + filtered
    }

    private var tabFocusedEntries: [TabFocusedTabOrder.Entry] {
        TabFocusedTabOrder.entries(
            appState: appState,
            projectStore: projectStore,
            projectGroupStore: projectGroupStore,
            worktreeStore: worktreeStore
        )
    }

    private func selectGlobalTab(index: Int) -> Bool {
        let entries = tabFocusedEntries
        guard index >= 0, index < entries.count else { return false }
        return selectGlobalTab(entries[index])
    }

    private func selectGlobalTabRelative(offset: Int) -> Bool {
        let entries = tabFocusedEntries
        guard entries.count > 1 else { return false }
        guard let projectID = appState.activeProjectID,
              let area = appState.focusedArea(for: projectID),
              let activeTabID = area.activeTabID,
              let current = entries.firstIndex(where: { $0.areaID == area.id && $0.tabID == activeTabID })
        else { return false }
        let next = (current + offset + entries.count) % entries.count
        return selectGlobalTab(entries[next])
    }

    private func selectGlobalTab(_ entry: TabFocusedTabOrder.Entry) -> Bool {
        if appState.activeProjectID != entry.projectID,
           let project = navigableProjects.first(where: { $0.id == entry.projectID })
        {
            worktreeStore.ensurePrimary(for: project)
            if let worktree = worktreeStore.preferred(for: project.id, matching: appState.activeWorktreeID[project.id]) {
                appState.selectProject(project, worktree: worktree)
            }
        }
        if let worktreeID = entry.worktreeID,
           appState.activeWorktreeID[entry.projectID] != worktreeID,
           let worktree = worktreeStore.worktree(projectID: entry.projectID, worktreeID: worktreeID)
        {
            appState.selectWorktree(projectID: entry.projectID, worktree: worktree)
        }
        appState.dispatch(.selectTab(projectID: entry.projectID, areaID: entry.areaID, tabID: entry.tabID))
        return true
    }

    func perform(_ action: ShortcutAction, activeProject: Project?) -> Bool {
        if let index = action.tabSelectionIndex {
            if AppLayoutStore.shared.layout == .tabFocused {
                return selectGlobalTab(index: index)
            }
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
        case .newHomeTab:
            return HomeProjectService.openHomeTab(
                appState: appState,
                worktreeStore: worktreeStore,
                projectGroupStore: projectGroupStore
            )
        case .newBrowserTab:
            return appState.openInBuiltInBrowser(BrowserURL.homeURL)
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
            if AppLayoutStore.shared.layout == .tabFocused, selectGlobalTabRelative(offset: 1) {
                return true
            }
            guard let projectID = appState.activeProjectID else { return false }
            appState.selectNextTab(projectID: projectID)
            return true
        case .previousTab:
            if AppLayoutStore.shared.layout == .tabFocused, selectGlobalTabRelative(offset: -1) {
                return true
            }
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
        case .recentlyRemovedProjects:
            postTerminalOmnibox(scope: .recentlyRemovedProjects)
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
                    worktreeStore: worktreeStore,
                    projectGroupStore: projectGroupStore
                )
            }
            return true
        case .createWorktree:
            guard activeProject != nil else { return false }
            notificationCenter.post(name: .createWorktreeRequested, object: nil)
            return true
        case .removeCurrentWorktree:
            guard activeProject != nil else { return false }
            notificationCenter.post(name: .removeCurrentWorktreeRequested, object: nil)
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
        case .terminalOmniboxWorkspaces:
            postTerminalOmnibox(scope: .workspaces)
            return true
        case .terminalOmniboxCommands:
            postTerminalOmnibox(scope: .commandShortcuts)
            return true
        case .toggleSidebar:
            notificationCenter.post(name: .toggleSidebar, object: nil)
            return true
        case .toggleAppLayout:
            AppLayoutStore.shared.toggle()
            return true
        case .toggleExtensionConsole:
            notificationCenter.post(name: .toggleExtensionConsole, object: nil)
            return true
        case .inspectElement:
            return appState.inspectActiveBrowserElement()
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
