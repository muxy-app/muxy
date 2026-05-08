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

    private enum FileTreeLayout {
        static let minWidth: CGFloat = 180
        static let defaultWidth: CGFloat = 260
        static let maxWidth: CGFloat = 600
    }

    private enum RichInputPanelLayout {
        static let minWidth: CGFloat = 280
        static let defaultWidth: CGFloat = 380
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
    @State private var fileTreePanelVisible = false
    @AppStorage("muxy.fileTreeWidth") private var fileTreePanelWidth: Double = .init(FileTreeLayout.defaultWidth)
    @State private var fileTreeStates: [WorktreeKey: FileTreeState] = [:]
    @State private var richInputPanelVisible = false
    @AppStorage("muxy.richInputPanelWidth") private var richInputPanelWidth: Double = .init(RichInputPanelLayout.defaultWidth)
    @AppStorage(RichInputPreferences.fontSizeKey) private var richInputFontSize: Double = RichInputPreferences.defaultFontSize
    @State private var richInputStates: [WorktreeKey: RichInputState] = [:]
    @State private var showQuickOpen = false
    @State private var showFindInFiles = false
    @State private var showWorktreeSwitcher = false
    @State private var overlayAnimatingOut = false
    @State private var isFullScreen = false
    @State private var sidebarExpanded = UserDefaults.standard.bool(forKey: "muxy.sidebarExpanded")
    @AppStorage(SidebarCollapsedStyle.storageKey) private var sidebarCollapsedStyleRaw = SidebarCollapsedStyle.defaultValue.rawValue
    @AppStorage(SidebarExpandedStyle.storageKey) private var sidebarExpandedStyleRaw = SidebarExpandedStyle.defaultValue.rawValue
    @AppStorage("muxy.notifications.toastPosition") private var toastPositionRaw = ToastPosition.topCenter.rawValue
    @MainActor private var trafficLightWidth: CGFloat { UIMetrics.scaled(75) }

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 0) {
                if !isFullScreen {
                    Color.clear
                        .frame(width: topBarLeadingWidth)
                        .fixedSize(horizontal: true, vertical: false)
                        .overlay(alignment: .trailing) {
                            HStack(spacing: 0) {
                                navigationArrows
                                Rectangle().fill(MuxyTheme.border).frame(width: 1)
                            }
                        }
                }
                topBarContent
            }
            .frame(height: UIMetrics.scaled(32))
            .background(WindowDragRepresentable())
            .background(MuxyTheme.bg)

            Rectangle().fill(MuxyTheme.border).frame(height: 1)
                .background(MuxyTheme.bg)

            HStack(spacing: 0) {
                HStack(spacing: 0) {
                    Sidebar()
                    if !SidebarLayout.isHidden(expanded: sidebarExpanded, collapsedStyle: sidebarCollapsedStyle) {
                        Rectangle().fill(MuxyTheme.border).frame(width: 1)
                            .accessibilityHidden(true)
                    }
                }
                .fixedSize(horizontal: true, vertical: false)
                .background(MuxyTheme.bg)

                VStack(spacing: 0) {
                    HStack(spacing: 0) {
                        ZStack {
                            MuxyTheme.bg
                            if let project = activeProject,
                               appState.workspaceRoot(for: project.id) == nil,
                               let worktree = resolvedActiveWorktree(for: project)
                            {
                                EmptyProjectPlaceholder(project: project) {
                                    appState.selectWorktree(projectID: project.id, worktree: worktree)
                                }
                            } else if projectsWithWorkspaces.isEmpty {
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

                        rightSidePanel
                    }

                    ProjectStatusBar(
                        activePane: activeTerminalPane,
                        isInteractive: activeTerminalPane != nil && !overlayAnimatingOut,
                        richInputVisible: richInputPanelVisible,
                        richInputFontSize: $richInputFontSize
                    )
                }
            }
        }
        .environment(\.overlayActive, showQuickOpen || showFindInFiles || showWorktreeSwitcher || overlayAnimatingOut)
        .overlay(alignment: toastAlignment) {
            if let toast = ToastState.shared.message {
                HStack(spacing: UIMetrics.spacing3) {
                    Image(systemName: "checkmark.circle.fill")
                        .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                        .foregroundStyle(MuxyTheme.diffAddFg)
                    Text(toast)
                        .font(.system(size: UIMetrics.fontBody, weight: .medium))
                        .foregroundStyle(MuxyTheme.fg)
                }
                .padding(.horizontal, UIMetrics.scaled(14))
                .padding(.vertical, UIMetrics.spacing4)
                .background(MuxyTheme.bg, in: Capsule())
                .overlay(Capsule().stroke(MuxyTheme.border, lineWidth: 1))
                .padding(toastEdgePadding)
                .transition(.move(edge: toastTransitionEdge).combined(with: .opacity))
                .allowsHitTesting(false)
                .accessibilityLabel(toast)
                .accessibilityAddTraits(.isStaticText)
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
            if showFindInFiles, let project = activeProject {
                FindInFilesOverlay(
                    projectPath: activeWorktreePath(for: project),
                    onSelect: { match in
                        showFindInFiles = false
                        appState.openFile(
                            match.absolutePath,
                            projectID: project.id,
                            line: match.lineNumber,
                            column: match.column
                        )
                    },
                    onDismiss: { showFindInFiles = false }
                )
                .transition(.opacity.combined(with: .scale(scale: 0.98)))
            }
        }
        .overlay {
            if showWorktreeSwitcher {
                OpenerOverlay(
                    items: openerItems,
                    recents: openerRecentItems,
                    activeWorktreeKey: activeWorktreeKey,
                    onSelect: { item in
                        showWorktreeSwitcher = false
                        handleOpenerSelection(item)
                    },
                    onDismiss: { showWorktreeSwitcher = false }
                )
                .transition(.opacity.combined(with: .scale(scale: 0.98)))
            }
        }
        .animation(.easeInOut(duration: 0.15), value: showQuickOpen)
        .animation(.easeInOut(duration: 0.15), value: showFindInFiles)
        .animation(.easeInOut(duration: 0.15), value: showWorktreeSwitcher)
        .modifier(OverlayExitTracker(
            showQuickOpen: showQuickOpen,
            showFindInFiles: showFindInFiles,
            showWorktreeSwitcher: showWorktreeSwitcher,
            onAnimatingOut: { overlayAnimatingOut = $0 }
        ))
        .animation(.easeInOut(duration: 0.2), value: ToastState.shared.message != nil)
        .coordinateSpace(name: DragCoordinateSpace.mainWindow)
        .environment(dragCoordinator)
        .background(MainWindowShortcutInterceptor(
            onShortcut: { action in handleShortcutAction(action) },
            onCommandShortcut: { shortcut in handleCommandShortcut(shortcut) },
            onMouseBack: { appState.goBack() },
            onMouseForward: { appState.goForward() }
        ))
        .background(WindowConfigurator(configVersion: ghostty.configVersion, uiScalePreset: UIScale.shared.preset))
        .background(WindowTitleUpdater(title: windowTitle))
        .ignoresSafeArea(.container, edges: .top)
        .onReceive(NotificationCenter.default.publisher(for: .quickOpen)) { _ in
            showQuickOpen.toggle()
        }
        .onReceive(NotificationCenter.default.publisher(for: .findInFiles)) { _ in
            showFindInFiles.toggle()
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
        .background(WindowOpenReceiver(openWindow: openWindow))
        .modifier(SidePanelNotificationListeners(
            onToggleAttachedVCS: { toggleAttachedVCSPanel() },
            onToggleFileTree: { toggleFileTreePanel() },
            onToggleRichInput: { toggleRichInputPanel() }
        ))
        .onChange(of: vcsPruneSignature) {
            pruneFileTreeStates()
        }
        .onChange(of: vcsEnsureSignature) {
            guard let project = activeProject else { return }
            if fileTreePanelVisible {
                ensureFileTreeState(for: project)
            }
        }
        .modifier(FileTreeSelectionSync(
            filePath: activeEditorFilePath,
            panelVisible: fileTreePanelVisible,
            sync: syncFileTreeSelection
        ))
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
        .onChange(of: appState.pendingLayoutApply != nil) { _, isPresented in
            guard isPresented, let pending = appState.pendingLayoutApply else { return }
            presentLayoutApplyConfirmation(pending: pending)
        }
    }

    private var navigationArrows: some View {
        HStack(spacing: UIMetrics.spacing1) {
            NavigationArrowButton(
                symbol: "chevron.left",
                isEnabled: appState.navigation.canGoBack,
                label: "Back (\(KeyBindingStore.shared.combo(for: .navigateBack).displayString))"
            ) {
                appState.goBack()
            }
            NavigationArrowButton(
                symbol: "chevron.right",
                isEnabled: appState.navigation.canGoForward,
                label: "Forward (\(KeyBindingStore.shared.combo(for: .navigateForward).displayString))"
            ) {
                appState.goForward()
            }
        }
        .padding(.trailing, UIMetrics.spacing2)
    }

    @ViewBuilder
    private var topBarContent: some View {
        if let project = activeProject,
           let root = appState.workspaceRoot(for: project.id),
           case let .tabArea(area) = root
        {
            PaneTabStrip(
                areaID: area.id,
                tabs: PaneTabStrip.snapshots(from: area.tabs),
                activeTabID: area.activeTabID,
                isFocused: true,
                isWindowTitleBar: true,
                showVCSButton: true,
                showDevelopmentBadge: AppEnvironment.isDevelopment,
                openInIDEProjectPath: activeWorktreePath(for: project),
                openInIDEFilePath: area.activeTab?.content.editorState?.filePath,
                openInIDECursorProvider: {
                    guard let editorState = appState.activeTab(for: project.id)?.content.editorState else {
                        return (nil, nil)
                    }
                    return (editorState.cursorLine, editorState.cursorColumn)
                },
                projectID: project.id,
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
                onCloseOtherTabs: { tabID in
                    for id in area.tabs.filter({ $0.id != tabID && !$0.isPinned }).map(\.id) {
                        appState.closeTab(id, areaID: area.id, projectID: project.id)
                    }
                },
                onCloseTabsToLeft: { tabID in
                    guard let index = area.tabs.firstIndex(where: { $0.id == tabID }) else { return }
                    for id in area.tabs.prefix(index).filter({ !$0.isPinned }).map(\.id) {
                        appState.closeTab(id, areaID: area.id, projectID: project.id)
                    }
                },
                onCloseTabsToRight: { tabID in
                    guard let index = area.tabs.firstIndex(where: { $0.id == tabID }) else { return }
                    for id in area.tabs.suffix(from: index + 1).filter({ !$0.isPinned }).map(\.id) {
                        appState.closeTab(id, areaID: area.id, projectID: project.id)
                    }
                },
                onSplit: { dir in
                    appState.dispatch(.splitArea(.init(
                        projectID: project.id,
                        areaID: area.id,
                        direction: dir,
                        position: .second
                    )))
                },
                onDropAction: { result in
                    appState.dispatch(result.action(projectID: project.id))
                },
                onCreateTabAdjacent: { tabID, side in
                    area.createTabAdjacent(to: tabID, side: side)
                },
                onTogglePin: { tabID in
                    area.togglePin(tabID)
                },
                onSetCustomTitle: { tabID, title in
                    area.setCustomTitle(tabID, title: title)
                    appState.saveWorkspaces()
                },
                onSetColorID: { tabID, colorID in
                    area.setColorID(tabID, colorID: colorID)
                    appState.saveWorkspaces()
                },
                onReorderTab: { fromOffsets, toOffset in
                    area.reorderTab(fromOffsets: fromOffsets, toOffset: toOffset)
                }
            )
        } else {
            WindowDragRepresentable(alwaysEnabled: true)
                .overlay {
                    HStack {
                        if let project = activeProject {
                            Text(project.name)
                                .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                                .foregroundStyle(MuxyTheme.fgMuted)
                                .padding(.leading, UIMetrics.spacing6)
                        }
                        Spacer(minLength: 0)
                    }
                    .allowsHitTesting(false)
                }
                .overlay(alignment: .trailing) {
                    HStack(spacing: 0) {
                        if AppEnvironment.isDevelopment {
                            devModeBadge
                                .padding(.trailing, UIMetrics.spacing3)
                        }
                        if let project = activeProject {
                            OpenInIDEControl(
                                projectPath: activeWorktreePath(for: project),
                                filePath: activeEditorFilePath,
                                cursorProvider: activeEditorCursor
                            )
                            LayoutPickerMenu(projectID: project.id)
                        }
                        if let version = UpdateService.shared.availableUpdateVersion {
                            UpdateBadge(version: version) {
                                UpdateService.shared.checkForUpdates()
                            }
                            .padding(.trailing, UIMetrics.spacing2)
                        }
                        if let project = activeProject, activeProjectHasSplitWorkspace {
                            IconButton(symbol: "doc.text", size: 12, accessibilityLabel: "Quick Open") {
                                NotificationCenter.default.post(name: .quickOpen, object: nil)
                            }
                            .help("Quick Open (\(KeyBindingStore.shared.combo(for: .quickOpen).displayString))")
                            FileDiffIconButton {
                                openVCS(for: project)
                            }
                            FileTreeIconButton {
                                NotificationCenter.default.post(name: .toggleFileTree, object: nil)
                            }
                            .help("File Tree (\(KeyBindingStore.shared.combo(for: .toggleFileTree).displayString))")
                        }
                    }
                    .padding(.trailing, UIMetrics.spacing2)
                }
        }
    }

    private var openerItems: [OpenerItem] {
        var items: [OpenerItem] = []

        for project in projectStore.projects {
            items.append(.project(.init(
                projectID: project.id,
                projectName: project.name
            )))
        }

        for project in projectStore.projects {
            for worktree in worktreeStore.list(for: project.id) {
                items.append(.worktree(.init(
                    projectID: project.id,
                    projectName: project.name,
                    worktreeID: worktree.id,
                    worktreeName: worktree.isPrimary && worktree.name.isEmpty ? "main" : worktree.name,
                    branch: worktree.branch,
                    isPrimary: worktree.isPrimary
                )))
            }
        }

        if let active = activeProject {
            for descriptor in appState.availableLayouts(for: active.id) {
                items.append(.layout(.init(
                    projectID: active.id,
                    projectName: active.name,
                    layoutName: descriptor.name
                )))
            }

            let worktrees = worktreeStore.list(for: active.id)
            for branch in BranchCache.shared.branches(for: active.path) {
                let matching = worktrees.first { $0.branch == branch }
                items.append(.branch(.init(
                    projectID: active.id,
                    projectName: active.name,
                    branch: branch,
                    matchingWorktreeID: matching?.id
                )))
            }

            for area in appState.allAreas(for: active.id) {
                for tab in area.tabs {
                    items.append(.openTab(.init(
                        projectID: active.id,
                        projectName: active.name,
                        areaID: area.id,
                        tabID: tab.id,
                        title: tab.title,
                        kind: tab.kind.rawValue
                    )))
                }
            }
        }

        return items
    }

    private var openerRecentItems: [OpenerItem] {
        let allByID = Dictionary(uniqueKeysWithValues: openerItems.map { ($0.id, $0) })
        return OpenerPreferences.recents.compactMap { allByID[$0.key] }
    }

    private func handleOpenerSelection(_ item: OpenerItem) {
        OpenerPreferences.remember(.init(key: item.id, category: item.category))
        switch item {
        case let .project(project):
            guard let target = projectStore.projects.first(where: { $0.id == project.projectID }) else { return }
            let worktree = worktreeStore.preferred(for: target.id, matching: appState.activeWorktreeID[target.id])
            if let worktree {
                appState.selectProject(target, worktree: worktree)
            }
        case let .worktree(wt):
            guard let target = projectStore.projects.first(where: { $0.id == wt.projectID }),
                  let worktree = worktreeStore.list(for: wt.projectID).first(where: { $0.id == wt.worktreeID })
            else { return }
            if appState.activeProjectID == wt.projectID {
                appState.selectWorktree(projectID: wt.projectID, worktree: worktree)
            } else {
                appState.selectProject(target, worktree: worktree)
            }
        case let .layout(layout):
            appState.requestApplyLayout(projectID: layout.projectID, layoutName: layout.layoutName)
        case let .branch(br):
            if let worktreeID = br.matchingWorktreeID,
               let worktree = worktreeStore.list(for: br.projectID).first(where: { $0.id == worktreeID }),
               let project = projectStore.projects.first(where: { $0.id == br.projectID })
            {
                if appState.activeProjectID == br.projectID {
                    appState.selectWorktree(projectID: br.projectID, worktree: worktree)
                } else {
                    appState.selectProject(project, worktree: worktree)
                }
            } else {
                ToastState.shared.show("No worktree for '\(br.branch)'")
            }
        case let .openTab(tab):
            appState.dispatch(.selectTab(projectID: tab.projectID, areaID: tab.areaID, tabID: tab.tabID))
        }
    }

    private var toastPosition: ToastPosition {
        ToastPosition(rawValue: toastPositionRaw) ?? .topCenter
    }

    private var toastAlignment: Alignment {
        switch toastPosition {
        case .topCenter: .top
        case .topRight: .topTrailing
        case .bottomCenter: .bottom
        case .bottomRight: .bottomTrailing
        }
    }

    private var toastEdgePadding: EdgeInsets {
        let big = UIMetrics.scaled(40)
        let small = UIMetrics.spacing7
        return switch toastPosition {
        case .topCenter: EdgeInsets(top: big, leading: 0, bottom: 0, trailing: 0)
        case .topRight: EdgeInsets(top: big, leading: 0, bottom: 0, trailing: small)
        case .bottomCenter: EdgeInsets(top: 0, leading: 0, bottom: small, trailing: 0)
        case .bottomRight: EdgeInsets(top: 0, leading: 0, bottom: small, trailing: small)
        }
    }

    private var toastTransitionEdge: Edge {
        switch toastPosition {
        case .topCenter,
             .topRight: .top
        case .bottomCenter,
             .bottomRight: .bottom
        }
    }

    private var sidebarCollapsedStyle: SidebarCollapsedStyle {
        SidebarCollapsedStyle(rawValue: sidebarCollapsedStyleRaw) ?? .defaultValue
    }

    private var sidebarExpandedStyle: SidebarExpandedStyle {
        SidebarExpandedStyle(rawValue: sidebarExpandedStyleRaw) ?? .defaultValue
    }

    private var topBarLeadingWidth: CGFloat {
        let sidebarWidth = SidebarLayout.resolvedWidth(
            expanded: sidebarExpanded,
            collapsedStyle: sidebarCollapsedStyle,
            expandedStyle: sidebarExpandedStyle
        ) + 1
        let navigationMinimum = trafficLightWidth + navigationArrowsWidth
        return max(navigationMinimum, sidebarWidth)
    }

    private var navigationArrowsWidth: CGFloat { UIMetrics.scaled(52) }

    private var devModeBadge: some View {
        DebugButton()
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

    private var windowTitle: String {
        guard let project = activeProject else { return "Muxy" }
        guard let tabTitle = appState.activeTab(for: project.id)?.title,
              !tabTitle.isEmpty
        else { return project.name }
        return "\(project.name) — \(tabTitle)"
    }

    private var activeProjectWithWorkspace: Project? {
        guard let project = activeProject,
              appState.workspaceRoot(for: project.id) != nil
        else { return nil }
        return project
    }

    private func resolvedActiveWorktree(for project: Project) -> Worktree? {
        worktreeStore.preferred(for: project.id, matching: appState.activeWorktreeID[project.id])
    }

    private var shortcutDispatcher: ShortcutActionDispatcher {
        ShortcutActionDispatcher(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            ghostty: ghostty
        )
    }

    private func mountedWorktreeKeys(for project: Project) -> [WorktreeKey] {
        appState.workspaceRoots.keys
            .filter { $0.projectID == project.id }
            .sorted { $0.worktreeID.uuidString < $1.worktreeID.uuidString }
    }

    private func handleShortcutAction(_ action: ShortcutAction) -> Bool {
        shortcutDispatcher.perform(action, activeProject: activeProject) { project in
            openVCS(for: project)
        }
    }

    private func handleCommandShortcut(_ shortcut: CommandShortcut) -> Bool {
        guard let projectID = appState.activeProjectID,
              appState.workspaceRoot(for: projectID) != nil,
              !shortcut.trimmedCommand.isEmpty
        else { return false }
        appState.createCommandTab(projectID: projectID, shortcut: shortcut)
        return true
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

    @ViewBuilder
    private var rightSidePanel: some View {
        if richInputPanelVisible,
           let richInputState = activeRichInputState,
           let worktreeKey = activeWorktreeKey
        {
            HStack(spacing: 0) {
                sidePanelResizeHandle { delta in
                    let next = richInputPanelWidth - Double(delta)
                    richInputPanelWidth = max(
                        Double(RichInputPanelLayout.minWidth),
                        min(Double(RichInputPanelLayout.maxWidth), next)
                    )
                }
                RichInputSidePanel(
                    state: richInputState,
                    worktreeKey: worktreeKey,
                    onDismiss: { closeRichInputPanel() },
                    onSubmit: { appendReturn in submitRichInput(richInputState, appendReturn: appendReturn) }
                )
                .frame(width: CGFloat(richInputPanelWidth))
            }
        } else if vcsPanelVisible, VCSDisplayMode.current == .attached, let state = activeVCSState {
            HStack(spacing: 0) {
                sidePanelResizeHandle { delta in
                    vcsPanelWidth = max(
                        AttachedVCSLayout.minWidth,
                        min(AttachedVCSLayout.maxWidth, vcsPanelWidth - delta)
                    )
                }
                VCSTabView(state: state, focused: false, onFocus: {})
                    .frame(width: vcsPanelWidth)
            }
        } else if fileTreePanelVisible, let treeState = activeFileTreeState {
            HStack(spacing: 0) {
                sidePanelResizeHandle { delta in
                    let next = fileTreePanelWidth - Double(delta)
                    fileTreePanelWidth = max(
                        Double(FileTreeLayout.minWidth),
                        min(Double(FileTreeLayout.maxWidth), next)
                    )
                }
                FileTreeView(
                    state: treeState,
                    onOpenFile: { filePath in
                        guard let projectID = appState.activeProjectID else { return }
                        appState.openFile(filePath, projectID: projectID, preserveFocus: true)
                    },
                    onOpenTerminal: { directory in
                        guard let projectID = appState.activeProjectID else { return }
                        appState.dispatch(.createTabInDirectory(
                            projectID: projectID,
                            areaID: nil,
                            directory: directory
                        ))
                    },
                    onFileMoved: { oldPath, newPath in
                        appState.handleFileMoved(from: oldPath, to: newPath)
                    }
                )
                .id(treeState.rootPath)
                .frame(width: CGFloat(fileTreePanelWidth))
            }
        }
    }

    private func sidePanelResizeHandle(onDrag: @escaping (CGFloat) -> Void) -> some View {
        Rectangle().fill(MuxyTheme.border).frame(width: 1)
            .accessibilityHidden(true)
            .overlay {
                Color.clear
                    .frame(width: UIMetrics.scaled(5))
                    .contentShape(Rectangle())
                    .gesture(
                        DragGesture(minimumDistance: 1)
                            .onChanged { v in onDrag(v.translation.width) }
                    )
                    .onHover { on in
                        if on { NSCursor.resizeLeftRight.push() } else { NSCursor.pop() }
                    }
            }
    }

    private var activeFileTreeState: FileTreeState? {
        guard let project = activeProject,
              let key = appState.activeWorktreeKey(for: project.id)
        else { return nil }
        return fileTreeStates[key]
    }

    private func ensureFileTreeState(for project: Project) {
        guard let key = appState.activeWorktreeKey(for: project.id) else { return }
        let path = activeWorktreePath(for: project)
        if let existing = fileTreeStates[key], existing.rootPath == path { return }
        fileTreeStates[key] = FileTreeState(rootPath: path)
    }

    private var activeEditorState: EditorTabState? {
        guard let project = activeProject else { return nil }
        return appState.activeTab(for: project.id)?.content.editorState
    }

    private var activeEditorFilePath: String? {
        activeEditorState?.filePath
    }

    private func activeEditorCursor() -> (line: Int?, column: Int?) {
        guard let state = activeEditorState else { return (nil, nil) }
        return (state.cursorLine, state.cursorColumn)
    }

    private func syncFileTreeSelection(filePath: String?) {
        guard fileTreePanelVisible,
              let project = activeProject,
              let key = appState.activeWorktreeKey(for: project.id),
              let state = fileTreeStates[key]
        else { return }
        if let filePath {
            state.revealFile(at: filePath)
        } else {
            state.clearSelection()
        }
    }

    private func pruneFileTreeStates() {
        let validKeys = validVCSKeys()
        fileTreeStates = fileTreeStates.filter { validKeys.contains($0.key) }
        richInputStates = richInputStates.filter { validKeys.contains($0.key) }
    }

    private func toggleAttachedVCSPanel() {
        guard VCSDisplayMode.current == .attached,
              activeProject != nil
        else {
            vcsPanelVisible = false
            return
        }

        let isShowing = !vcsPanelVisible
        vcsPanelVisible = isShowing
        if isShowing {
            fileTreePanelVisible = false
            closeRichInputPanel()
        }
    }

    private func toggleFileTreePanel() {
        guard let project = activeProject else {
            if fileTreePanelVisible {
                fileTreePanelVisible = false
                NotificationCenter.default.post(name: .refocusActiveTerminal, object: nil)
            }
            return
        }

        ensureFileTreeState(for: project)
        let isShowing = !fileTreePanelVisible
        fileTreePanelVisible = isShowing
        if isShowing {
            vcsPanelVisible = false
            closeRichInputPanel()
        } else {
            NotificationCenter.default.post(name: .refocusActiveTerminal, object: nil)
        }
    }

    private var activeRichInputState: RichInputState? {
        guard let project = activeProject,
              let key = appState.activeWorktreeKey(for: project.id)
        else { return nil }
        if let existing = richInputStates[key] { return existing }
        let new = RichInputState()
        if let draft = RichInputDraftStore.shared.draft(for: key) {
            new.apply(draft)
        }
        richInputStates[key] = new
        return new
    }

    private var activeRichInputPaneID: UUID? {
        activeTerminalPane?.id
    }

    private var activeTerminalPane: TerminalPaneState? {
        guard let project = activeProject else { return nil }
        return appState.activeTab(for: project.id)?.content.pane
    }

    private func toggleRichInputPanel() {
        guard let richInputState = activeRichInputState else { return }
        guard richInputPanelVisible else {
            richInputPanelVisible = true
            richInputState.focusVersion += 1
            vcsPanelVisible = false
            fileTreePanelVisible = false
            return
        }
        if NSApp.keyWindow?.firstResponder is MarkdownEditingTextView {
            closeRichInputPanel()
        } else {
            richInputState.focusVersion += 1
        }
    }

    private func closeRichInputPanel() {
        richInputPanelVisible = false
        guard let paneID = activeRichInputPaneID,
              let view = TerminalViewRegistry.shared.existingView(for: paneID)
        else { return }
        DispatchQueue.main.async {
            view.window?.makeFirstResponder(view)
        }
    }

    private func submitRichInput(_ richInput: RichInputState, appendReturn: Bool) {
        guard let paneID = activeRichInputPaneID else { return }
        RichInputSubmitter.submit(richInput: richInput, paneID: paneID, appendReturn: appendReturn)
    }

    private var activeVCSState: VCSTabState? {
        guard let project = activeProject,
              appState.activeWorktreeKey(for: project.id) != nil
        else { return nil }
        return VCSStateStore.shared.state(for: activeWorktreePath(for: project))
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
                toggleAttachedVCSPanel()
            }
        )
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

        if kind == .runningProcess {
            alert.showsSuppressionButton = true
            alert.suppressionButton?.title = "Don't ask again"
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
                    if alert.suppressionButton?.state == .on {
                        TabCloseConfirmationPreferences.confirmRunningProcess = false
                    }
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

    private func presentLayoutApplyConfirmation(pending: AppState.PendingLayoutApply) {
        guard let window = NSApp.keyWindow ?? NSApp.mainWindow,
              window.attachedSheet == nil
        else {
            appState.cancelApplyLayout()
            return
        }

        let alert = NSAlert()
        alert.messageText = "Apply Layout '\(pending.layoutName)'?"
        alert.informativeText = "All terminals and tabs in this worktree will be closed and replaced with the layout."
        alert.alertStyle = .warning
        alert.icon = NSApp.applicationIconImage
        alert.addButton(withTitle: "Apply")
        alert.addButton(withTitle: "Cancel")
        alert.buttons[0].keyEquivalent = "\r"
        alert.buttons[1].keyEquivalent = "\u{1b}"

        alert.beginSheetModal(for: window) { response in
            if response == .alertFirstButtonReturn {
                appState.confirmApplyLayout()
            } else {
                appState.cancelApplyLayout()
            }
        }
    }
}

private struct WindowTitleUpdater: NSViewRepresentable {
    let title: String

    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        DispatchQueue.main.async {
            view.window?.title = title
        }
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        guard let window = nsView.window, window.title != title else { return }
        window.title = title
    }
}

private struct FileTreeSelectionSync: ViewModifier {
    let filePath: String?
    let panelVisible: Bool
    let sync: (String?) -> Void

    func body(content: Content) -> some View {
        content
            .onChange(of: filePath) { _, newValue in
                sync(newValue)
            }
            .onChange(of: panelVisible) { _, visible in
                guard visible else { return }
                sync(filePath)
            }
    }
}

private struct NavigationArrowButton: View {
    let symbol: String
    let isEnabled: Bool
    let label: String
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                .foregroundStyle(foregroundColor)
                .frame(width: UIMetrics.scaled(22), height: UIMetrics.scaled(22))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(!isEnabled)
        .onHover { hovered = $0 }
        .help(label)
        .accessibilityLabel(label)
    }

    private var foregroundColor: Color {
        guard isEnabled else { return MuxyTheme.fgMuted.opacity(0.35) }
        return hovered ? MuxyTheme.fg : MuxyTheme.fgMuted
    }
}

private struct MainWindowShortcutInterceptor: NSViewRepresentable {
    let onShortcut: (ShortcutAction) -> Bool
    let onCommandShortcut: (CommandShortcut) -> Bool
    let onMouseBack: () -> Void
    let onMouseForward: () -> Void

    func makeNSView(context: Context) -> ShortcutInterceptingView {
        let view = ShortcutInterceptingView()
        view.onShortcut = onShortcut
        view.onCommandShortcut = onCommandShortcut
        view.onMouseBack = onMouseBack
        view.onMouseForward = onMouseForward
        return view
    }

    func updateNSView(_ nsView: ShortcutInterceptingView, context: Context) {
        nsView.onShortcut = onShortcut
        nsView.onCommandShortcut = onCommandShortcut
        nsView.onMouseBack = onMouseBack
        nsView.onMouseForward = onMouseForward
    }
}

private final class ShortcutInterceptingView: NSView {
    var onShortcut: ((ShortcutAction) -> Bool)?
    var onCommandShortcut: ((CommandShortcut) -> Bool)?
    var onMouseBack: (() -> Void)?
    var onMouseForward: (() -> Void)?
    private var mouseMonitor: Any?

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window == nil {
            removeMouseMonitor()
        } else {
            installMouseMonitorIfNeeded()
        }
    }

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        guard event.type == .keyDown,
              ShortcutContext.isMainWindow(window)
        else { return super.performKeyEquivalent(with: event) }

        let scopes = ShortcutContext.activeScopes(for: window)
        let layerWasActive = CommandShortcutStore.shared.isLayerActive
        if let shortcut = CommandShortcutStore.shared.shortcut(for: event, scopes: scopes) {
            CommandShortcutStore.shared.deactivateLayer()
            _ = onCommandShortcut?(shortcut)
            return true
        }

        if layerWasActive {
            CommandShortcutStore.shared.deactivateLayer()
            return true
        }

        if CommandShortcutStore.shared.matchesPrefix(event: event, scopes: scopes) {
            CommandShortcutStore.shared.activateLayer()
            return true
        }

        if let action = KeyBindingStore.shared.action(for: event, scopes: scopes) {
            if onShortcut?(action) == true {
                return true
            }
        }

        return super.performKeyEquivalent(with: event)
    }

    private func installMouseMonitorIfNeeded() {
        guard mouseMonitor == nil else { return }
        mouseMonitor = NSEvent.addLocalMonitorForEvents(matching: [.otherMouseDown, .swipe]) { [weak self] event in
            guard let self,
                  let window = self.window,
                  window.isKeyWindow,
                  ShortcutContext.isMainWindow(window)
            else { return event }
            return self.handleNavigationEvent(event)
        }
    }

    private func handleNavigationEvent(_ event: NSEvent) -> NSEvent? {
        switch event.type {
        case .otherMouseDown:
            switch event.buttonNumber {
            case 3:
                onMouseBack?()
                return nil
            case 4:
                onMouseForward?()
                return nil
            default:
                return event
            }
        case .swipe:
            if event.deltaX > 0 {
                onMouseBack?()
                return nil
            }
            if event.deltaX < 0 {
                onMouseForward?()
                return nil
            }
            return event
        default:
            return event
        }
    }

    private func removeMouseMonitor() {
        guard let mouseMonitor else { return }
        NSEvent.removeMonitor(mouseMonitor)
        self.mouseMonitor = nil
    }
}

private struct WindowOpenReceiver: View {
    let openWindow: OpenWindowAction

    var body: some View {
        Color.clear
            .frame(width: 0, height: 0)
            .onReceive(NotificationCenter.default.publisher(for: .openVCSWindow)) { _ in
                openWindow(id: "vcs")
            }
            .onReceive(NotificationCenter.default.publisher(for: .openHelpWindow)) { _ in
                openWindow(id: "help")
            }
    }
}

private struct SidePanelNotificationListeners: ViewModifier {
    let onToggleAttachedVCS: () -> Void
    let onToggleFileTree: () -> Void
    let onToggleRichInput: () -> Void

    func body(content: Content) -> some View {
        content
            .onReceive(NotificationCenter.default.publisher(for: .toggleAttachedVCS)) { _ in
                onToggleAttachedVCS()
            }
            .onReceive(NotificationCenter.default.publisher(for: .toggleFileTree)) { _ in
                onToggleFileTree()
            }
            .onReceive(NotificationCenter.default.publisher(for: .toggleRichInput)) { _ in
                onToggleRichInput()
            }
    }
}

private struct OverlayExitTracker: ViewModifier {
    let showQuickOpen: Bool
    let showFindInFiles: Bool
    let showWorktreeSwitcher: Bool
    let onAnimatingOut: (Bool) -> Void

    func body(content: Content) -> some View {
        content
            .onChange(of: showQuickOpen) { _, visible in trackExit(visible) }
            .onChange(of: showFindInFiles) { _, visible in trackExit(visible) }
            .onChange(of: showWorktreeSwitcher) { _, visible in trackExit(visible) }
    }

    private func trackExit(_ visible: Bool) {
        guard !visible else { return }
        onAnimatingOut(true)
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.15) {
            onAnimatingOut(false)
        }
    }
}
