import AppKit
import SwiftUI

struct MainWindow: View {
    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore
    @Environment(GhosttyService.self) private var ghostty
    @Environment(\.openWindow) private var openWindow
    @State private var dragCoordinator = TabDragCoordinator()
    private enum AttachedVCSLayout {
        static let minWidth: CGFloat = 200
        static let defaultWidth: CGFloat = 400
        static let maxWidth: CGFloat = 800
    }

    private enum CloseConfirmationKind {
        case lastTab
        case unsavedEditor
        case runningProcess

        var title: String {
            switch self {
            case .lastTab:
                "Close Project?"
            case .unsavedEditor:
                "Save Changes Before Closing?"
            case .runningProcess:
                "Close Tab?"
            }
        }

        var message: String {
            switch self {
            case .lastTab:
                "This is the last tab. Closing it will remove the project from the sidebar."
            case .unsavedEditor:
                "This file has unsaved changes. If you don't save, your changes will be lost."
            case .runningProcess:
                "A process is still running in this tab. Are you sure you want to close it?"
            }
        }
    }

    @State private var vcsPanelVisible = false
    @State private var vcsPanelWidth: CGFloat = AttachedVCSLayout.defaultWidth
    @State private var vcsStates: [WorktreeKey: VCSTabState] = [:]
    @State private var showQuickOpen = false
    @State private var showWorktreeSwitcher = false
    @State private var isFullScreen = false
    @State private var sidebarExpanded = UserDefaults.standard.bool(forKey: "muxy.sidebarExpanded")
    private let trafficLightWidth: CGFloat = 75

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 0) {
                if !isFullScreen {
                    Color.clear.frame(width: topBarLeadingWidth)
                        .overlay(alignment: .trailing) {
                            if sidebarExpanded {
                                Rectangle().fill(MuxyTheme.border).frame(width: 1)
                            }
                        }
                }
                topBarContent
            }
            .frame(height: 32)
            .background(WindowDragRepresentable())
            .background(MuxyTheme.bg)

            Rectangle().fill(MuxyTheme.border).frame(height: 1)
                .background(MuxyTheme.bg)

            HStack(spacing: 0) {
                HStack(spacing: 0) {
                    Sidebar()
                    Rectangle().fill(MuxyTheme.border).frame(width: 1)
                }
                .background(MuxyTheme.bg)

                ZStack {
                    MuxyTheme.bg
                    if projectsWithWorkspaces.isEmpty {
                        WelcomeView()
                    } else if let project = activeProjectWithWorkspace,
                              let activeKey = appState.activeWorktreeKey(for: project.id)
                    {
                        ForEach(mountedWorktreeKeys(for: project), id: \.self) { key in
                            TerminalArea(
                                project: project,
                                worktreeKey: key,
                                isActiveProject: key == activeKey
                            )
                            .opacity(key == activeKey ? 1 : 0)
                            .allowsHitTesting(key == activeKey)
                            .zIndex(key == activeKey ? 1 : 0)
                        }
                    }
                }

                if vcsPanelVisible, VCSDisplayMode.current == .attached, let state = activeVCSState {
                    HStack(spacing: 0) {
                        Rectangle().fill(MuxyTheme.border).frame(width: 1)
                            .overlay {
                                Color.clear
                                    .frame(width: 5)
                                    .contentShape(Rectangle())
                                    .gesture(
                                        DragGesture(minimumDistance: 1)
                                            .onChanged { v in
                                                let delta = v.translation.width
                                                vcsPanelWidth = max(
                                                    AttachedVCSLayout.minWidth,
                                                    min(AttachedVCSLayout.maxWidth, vcsPanelWidth - delta)
                                                )
                                            }
                                    )
                                    .onHover { on in
                                        if on { NSCursor.resizeLeftRight.push() } else { NSCursor.pop() }
                                    }
                            }
                        VCSTabView(state: state, focused: false, onFocus: {})
                            .frame(width: vcsPanelWidth)
                    }
                }
            }
        }
        .environment(\.overlayActive, showQuickOpen || showWorktreeSwitcher)
        .overlay(alignment: .bottom) {
            if let toast = ToastState.shared.message {
                HStack(spacing: 6) {
                    Image(systemName: "checkmark.circle.fill")
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(MuxyTheme.diffAddFg)
                    Text(toast)
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(MuxyTheme.fg)
                }
                .padding(.horizontal, 14)
                .padding(.vertical, 8)
                .background(MuxyTheme.surface, in: Capsule())
                .overlay(Capsule().stroke(MuxyTheme.border, lineWidth: 1))
                .padding(.bottom, 16)
                .transition(.move(edge: .bottom).combined(with: .opacity))
                .allowsHitTesting(false)
            }
        }
        .overlay {
            if showQuickOpen, let project = activeProject {
                QuickOpenOverlay(
                    projectPath: activeWorktreePath(for: project),
                    onSelect: { filePath in
                        showQuickOpen = false
                        appState.openFile(filePath, projectID: project.id)
                    },
                    onDismiss: { showQuickOpen = false }
                )
                .transition(.opacity.combined(with: .scale(scale: 0.98)))
            }
        }
        .overlay {
            if showWorktreeSwitcher {
                WorktreeSwitcherOverlay(
                    items: worktreeSwitcherItems,
                    activeKey: activeWorktreeKey,
                    onSelect: { item in
                        showWorktreeSwitcher = false
                        guard let project = projectStore.projects.first(where: { $0.id == item.projectID }) else { return }
                        if appState.activeProjectID == item.projectID {
                            appState.selectWorktree(projectID: item.projectID, worktree: item.worktree)
                        } else {
                            appState.selectProject(project, worktree: item.worktree)
                        }
                    },
                    onDismiss: { showWorktreeSwitcher = false }
                )
                .transition(.opacity.combined(with: .scale(scale: 0.98)))
            }
        }
        .animation(.easeInOut(duration: 0.15), value: showQuickOpen)
        .animation(.easeInOut(duration: 0.15), value: showWorktreeSwitcher)
        .animation(.easeInOut(duration: 0.2), value: ToastState.shared.message != nil)
        .coordinateSpace(name: DragCoordinateSpace.mainWindow)
        .environment(dragCoordinator)
        .background(WindowConfigurator(configVersion: ghostty.configVersion))
        .ignoresSafeArea(.container, edges: .top)
        .onReceive(NotificationCenter.default.publisher(for: .quickOpen)) { _ in
            showQuickOpen.toggle()
        }
        .onReceive(NotificationCenter.default.publisher(for: .switchWorktree)) { _ in
            showWorktreeSwitcher.toggle()
        }
        .onReceive(NotificationCenter.default.publisher(for: .toggleSidebar)) { _ in
            withAnimation(.easeInOut(duration: 0.2)) {
                sidebarExpanded.toggle()
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .windowFullScreenDidChange)) { notification in
            isFullScreen = notification.userInfo?["isFullScreen"] as? Bool ?? false
        }
        .onReceive(NotificationCenter.default.publisher(for: .openVCSWindow)) { _ in
            openWindow(id: "vcs")
        }
        .onReceive(NotificationCenter.default.publisher(for: .toggleAttachedVCS)) { _ in
            if let project = activeProject {
                ensureVCSState(for: project)
            }
            vcsPanelVisible.toggle()
        }
        .onChange(of: vcsPruneSignature) {
            pruneVCSStates()
        }
        .onChange(of: vcsEnsureSignature) {
            guard let project = activeProject else { return }
            guard vcsPanelVisible, VCSDisplayMode.current == .attached else { return }
            ensureVCSState(for: project)
        }
        .onChange(of: appState.pendingLastTabClose != nil) { _, isPresented in
            guard isPresented else { return }
            presentCloseConfirmation(.lastTab)
        }
        .onChange(of: appState.pendingUnsavedEditorTabClose != nil) { _, isPresented in
            guard isPresented else { return }
            presentCloseConfirmation(.unsavedEditor)
        }
        .onChange(of: appState.pendingProcessTabClose != nil) { _, isPresented in
            guard isPresented else { return }
            presentCloseConfirmation(.runningProcess)
        }
        .onChange(of: appState.pendingSaveErrorMessage != nil) { _, isPresented in
            guard isPresented, let message = appState.pendingSaveErrorMessage else { return }
            presentSaveErrorAlert(message: message)
        }
    }

    @ViewBuilder
    private var topBarContent: some View {
        if let project = activeProject,
           let root = appState.workspaceRoot(for: project.id),
           case let .tabArea(area) = root
        {
            PaneTabStrip(
                area: area,
                isFocused: true,
                isWindowTitleBar: true,
                showVCSButton: true,
                projectID: project.id,
                onFocus: {},
                onSelectTab: { tabID in
                    appState.dispatch(.selectTab(projectID: project.id, areaID: area.id, tabID: tabID))
                },
                onCreateTab: {
                    appState.dispatch(.createTab(projectID: project.id, areaID: area.id))
                },
                onCreateVCSTab: {
                    openVCS(for: project, preferredAreaID: area.id)
                },
                onCloseTab: { tabID in
                    appState.closeTab(tabID, areaID: area.id, projectID: project.id)
                },
                onSplit: { dir in
                    appState.dispatch(.splitArea(.init(
                        projectID: project.id,
                        areaID: area.id,
                        direction: dir,
                        position: .second
                    )))
                },
                onClose: {
                    appState.dispatch(.closeArea(projectID: project.id, areaID: area.id))
                },
                onDropAction: { result in
                    appState.dispatch(result.action(projectID: project.id))
                }
            )
        } else {
            WindowDragRepresentable(alwaysEnabled: true)
                .overlay {
                    HStack {
                        if let project = activeProject {
                            Text(project.name)
                                .font(.system(size: 12, weight: .semibold))
                                .foregroundStyle(MuxyTheme.fgMuted)
                                .padding(.leading, 12)
                        }
                        Spacer(minLength: 0)
                    }
                    .allowsHitTesting(false)
                }
                .overlay(alignment: .trailing) {
                    HStack(spacing: 0) {
                        if let version = UpdateService.shared.availableUpdateVersion {
                            UpdateBadge(version: version) {
                                UpdateService.shared.checkForUpdates()
                            }
                            .padding(.trailing, 4)
                        }
                        if let project = activeProject, activeProjectHasSplitWorkspace {
                            FileDiffIconButton {
                                openVCS(for: project)
                            }
                        }
                    }
                    .padding(.trailing, 4)
                }
        }
    }

    private var worktreeSwitcherItems: [WorktreeSwitcherItem] {
        projectStore.projects.flatMap { project in
            worktreeStore.list(for: project.id).map { worktree in
                WorktreeSwitcherItem(
                    projectID: project.id,
                    projectName: project.name,
                    worktree: worktree
                )
            }
        }
    }

    private var topBarLeadingWidth: CGFloat {
        let sidebarWidth = SidebarLayout.resolvedWidth(expanded: sidebarExpanded) + 1
        return max(trafficLightWidth, sidebarWidth)
    }

    private var activeWorktreeKey: WorktreeKey? {
        guard let projectID = appState.activeProjectID,
              let worktreeID = appState.activeWorktreeID[projectID]
        else { return nil }
        return WorktreeKey(projectID: projectID, worktreeID: worktreeID)
    }

    private var activeProject: Project? {
        guard let pid = appState.activeProjectID else { return nil }
        return projectStore.projects.first { $0.id == pid }
    }

    private var activeProjectWithWorkspace: Project? {
        guard let project = activeProject,
              appState.workspaceRoot(for: project.id) != nil
        else { return nil }
        return project
    }

    private func mountedWorktreeKeys(for project: Project) -> [WorktreeKey] {
        appState.workspaceRoots.keys
            .filter { $0.projectID == project.id }
            .sorted { $0.worktreeID.uuidString < $1.worktreeID.uuidString }
    }

    private var activeProjectHasSplitWorkspace: Bool {
        guard let project = activeProject,
              let root = appState.workspaceRoot(for: project.id)
        else { return false }
        if case .split = root { return true }
        return false
    }

    private var projectsWithWorkspaces: [Project] {
        projectStore.projects.filter { appState.workspaceRoot(for: $0.id) != nil }
    }

    private var activeVCSState: VCSTabState? {
        guard let project = activeProject,
              let key = appState.activeWorktreeKey(for: project.id)
        else { return nil }
        return vcsStates[key]
    }

    private func ensureVCSState(for project: Project) {
        guard let key = appState.activeWorktreeKey(for: project.id) else { return }
        guard vcsStates[key] == nil else { return }
        vcsStates[key] = VCSTabState(projectPath: activeWorktreePath(for: project))
    }

    private func activeWorktreePath(for project: Project) -> String {
        guard let key = appState.activeWorktreeKey(for: project.id) else { return project.path }
        return worktreeStore
            .worktree(projectID: project.id, worktreeID: key.worktreeID)?
            .path ?? project.path
    }

    private func openVCS(for project: Project, preferredAreaID: UUID? = nil) {
        VCSDisplayMode.current.route(
            tab: {
                let areaID = preferredAreaID
                    ?? appState.focusedAreaID(for: project.id)
                    ?? appState.workspaceRoot(for: project.id)?.allAreas().first?.id
                guard let areaID else { return }
                appState.dispatch(.createVCSTab(projectID: project.id, areaID: areaID))
            },
            window: { openWindow(id: "vcs") },
            attached: {
                ensureVCSState(for: project)
                vcsPanelVisible.toggle()
            }
        )
    }

    private func pruneVCSStates() {
        let validKeys = validVCSKeys()
        vcsStates = vcsStates.filter { validKeys.contains($0.key) }
    }

    private func validVCSKeys() -> Set<WorktreeKey> {
        var keys: Set<WorktreeKey> = []
        for project in projectStore.projects {
            for worktree in worktreeStore.list(for: project.id) {
                keys.insert(WorktreeKey(projectID: project.id, worktreeID: worktree.id))
            }
        }
        return keys
    }

    private var vcsPruneSignature: [String] {
        var result: [String] = []
        for project in projectStore.projects {
            result.append(project.id.uuidString)
            for worktree in worktreeStore.list(for: project.id) {
                result.append(worktree.id.uuidString)
            }
        }
        return result
    }

    private var vcsEnsureSignature: String {
        let projectID = appState.activeProjectID?.uuidString ?? ""
        let worktreeID = appState.activeProjectID.flatMap { appState.activeWorktreeID[$0] }?.uuidString ?? ""
        return "\(projectID):\(worktreeID)"
    }

    private func presentCloseConfirmation(_ kind: CloseConfirmationKind) {
        guard let window = NSApp.keyWindow ?? NSApp.mainWindow,
              window.attachedSheet == nil
        else { return }

        let alert = NSAlert()
        alert.messageText = kind.title
        alert.informativeText = kind.message
        alert.alertStyle = .warning
        alert.icon = NSApp.applicationIconImage

        switch kind {
        case .unsavedEditor:
            alert.addButton(withTitle: "Save")
            alert.addButton(withTitle: "Cancel")
            alert.addButton(withTitle: "Don't Save")
            alert.buttons[0].keyEquivalent = "\r"
            alert.buttons[1].keyEquivalent = "\u{1b}"
            alert.buttons[2].keyEquivalent = "d"
            alert.buttons[2].keyEquivalentModifierMask = [.command]
        case .lastTab,
             .runningProcess:
            alert.addButton(withTitle: "Close")
            alert.addButton(withTitle: "Cancel")
            alert.buttons[0].keyEquivalent = "\r"
            alert.buttons[1].keyEquivalent = "\u{1b}"
        }

        alert.beginSheetModal(for: window) { response in
            switch kind {
            case .lastTab:
                if response == .alertFirstButtonReturn {
                    appState.confirmCloseLastTab()
                } else {
                    appState.cancelCloseLastTab()
                }
            case .unsavedEditor:
                switch response {
                case .alertFirstButtonReturn:
                    appState.saveAndCloseUnsavedEditorTab()
                case .alertThirdButtonReturn:
                    appState.confirmCloseUnsavedEditorTab()
                default:
                    appState.cancelCloseUnsavedEditorTab()
                }
            case .runningProcess:
                if response == .alertFirstButtonReturn {
                    appState.confirmCloseRunningTab()
                } else {
                    appState.cancelCloseRunningTab()
                }
            }
        }
    }

    private func presentSaveErrorAlert(message: String) {
        guard let window = NSApp.keyWindow ?? NSApp.mainWindow,
              window.attachedSheet == nil
        else {
            appState.pendingSaveErrorMessage = nil
            return
        }

        let alert = NSAlert()
        alert.messageText = "Could Not Save File"
        alert.informativeText = message
        alert.alertStyle = .warning
        alert.icon = NSApp.applicationIconImage
        alert.addButton(withTitle: "OK")
        alert.buttons[0].keyEquivalent = "\r"

        alert.beginSheetModal(for: window) { _ in
            appState.pendingSaveErrorMessage = nil
        }
    }
}
