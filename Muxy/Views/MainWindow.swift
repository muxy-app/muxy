import AppKit
import SwiftUI

enum MainWindowLayout {
    static func leftNavigationWidth(sidebarWidth: CGFloat) -> CGFloat {
        max(0, sidebarWidth)
    }

    static func titleBarNavigationOverlayWidth(
        leftNavigationWidth: CGFloat,
        titleBarNavigationWidth: CGFloat,
        isFullScreen: Bool
    ) -> CGFloat {
        guard !isFullScreen else { return 0 }
        return max(leftNavigationWidth, titleBarNavigationWidth)
    }

    static func mainTitleBarLeadingInset(
        leftNavigationWidth: CGFloat,
        titleBarNavigationOverlayWidth: CGFloat,
        isFullScreen: Bool
    ) -> CGFloat {
        guard !isFullScreen else { return 0 }
        return max(0, titleBarNavigationOverlayWidth - leftNavigationWidth)
    }
}

struct MainWindow: View {
    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore
    @Environment(ProjectGroupStore.self) private var projectGroupStore
    @Environment(RemoteDeviceStore.self) private var remoteDeviceStore
    @Environment(BrowserProfileStore.self) private var browserProfileStore
    @Environment(GhosttyService.self) private var ghostty
    @AppStorage(BrowserPreferences.enabledKey) private var browserEnabled = true
    @State private var dragCoordinator = TabDragCoordinator()
    private enum CloseConfirmationKind {
        case lastTab
        case runningProcess

        var title: String {
            switch self {
            case .lastTab:
                "Close Project?"
            case .runningProcess:
                "Close Tab?"
            }
        }

        var message: String {
            switch self {
            case .lastTab:
                "This is the last tab. Closing it will remove the project from the sidebar."
            case .runningProcess:
                "A process is still running in this tab. Are you sure you want to close it?"
            }
        }
    }

    @State var panelHost = PanelHost.shared
    @State private var workspaceFileWatcher = WorkspaceFileWatcher()
    @AppStorage("muxy.extensionPanelWidth") private var extensionPanelWidth: Double = PanelLayoutMetrics.extensionDefaultWidth
    @AppStorage("muxy.extensionPanelHeight") private var extensionPanelHeight: Double = PanelLayoutMetrics.extensionDefaultHeight
    @AppStorage("muxy.richInputPanelWidth") private var richInputPanelWidth: Double = PanelLayoutMetrics.richInputDefaultWidth
    @AppStorage("muxy.richInputPanelHeight") private var richInputPanelHeight: Double = PanelLayoutMetrics.richInputDefaultHeight
    @AppStorage(RichInputPreferences.fontSizeKey) private var richInputFontSize: Double = RichInputPreferences.defaultFontSize
    @AppStorage(RichInputPreferences.floatingKey) private var richInputFloating = RichInputPreferences.defaultFloating
    @AppStorage(RichInputPreferences.positionKey) private var richInputPosition: PanelPosition = RichInputPreferences
        .defaultPosition
    @AppStorage(RichInputPreferences.broadcastKey) private var richInputBroadcast = RichInputPreferences.defaultBroadcast
    @State private var richInputStates: [WorktreeKey: RichInputState] = [:]
    @State private var visitedWorktreeKeys: Set<WorktreeKey> = []
    @AppStorage(WorktreeListPreferences.orderByMRUKey)
    private var orderWorktreesByMRU = WorktreeListPreferences.defaultOrderByMRU
    @State private var showTerminalOmnibox = false
    @State private var terminalOmniboxLaunchScope = TerminalOmniboxLaunchScope.openTabs
    @State private var showProjectPicker = false
    @State private var remoteProjectDevice: RemoteDevice?
    @State private var overlayAnimatingOut = false
    @State private var isFullScreen = false
    @AppStorage("muxy.sidebarExpanded") private var sidebarExpanded = false
    @AppStorage("muxy.showStatusBar") private var showStatusBar = true
    @AppStorage(HomeProjectPreferences.visibleKey) private var showHomeProject = HomeProjectPreferences.defaultVisible
    @AppStorage("muxy.extensionOutputSelected") private var extensionOutputSelectedStored = ""
    @AppStorage("muxy.extensionConsoleHeight") private var extensionConsoleHeight: Double = PanelLayoutMetrics.consoleDefaultHeight
    @State private var extensionOutputSelected: String?
    @AppStorage(SidebarCollapsedStyle.storageKey) private var sidebarCollapsedStyleRaw = SidebarCollapsedStyle.defaultValue.rawValue
    @AppStorage(SidebarExpandedStyle.storageKey) private var sidebarExpandedStyleRaw = SidebarExpandedStyle.defaultValue.rawValue
    @AppStorage("muxy.sidebarExpandedCustomWidth") private var sidebarExpandedCustomWidth: Double = .init(SidebarLayout.expandedWidth)
    @AppStorage(NotificationSettings.Key.toastPosition)
    private var toastPositionRaw = NotificationSettings.Default.toastPosition.rawValue
    @AppStorage(RecordingPreferences.autoSendKey) private var recordingAutoSend = RecordingPreferences.defaultAutoSend
    @AppStorage(RecordingPreferences.languageKey) private var recordingLanguage = RecordingPreferences.defaultLanguage
    @State private var voiceRecording = VoiceRecordingState.shared
    @MainActor private var trafficLightWidth: CGFloat { UIMetrics.scaled(75) }

    var body: some View {
        HStack(spacing: 0) {
            leftNavigationColumn
            if sidebarIsResizable {
                sidebarResizeHandle
            }
            mainWorkspaceColumn
        }
        .animation(.easeInOut(duration: 0.2), value: sidebarExpanded)
        .overlay(alignment: .topLeading) {
            titleBarNavigationOverlay
        }
        .environment(\.overlayActive, overlayActive)
        .overlay(alignment: .bottom) {
            if voiceRecording.isPanelVisible {
                VoiceRecordingPanel(state: voiceRecording, autoSend: recordingAutoSend)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .animation(.easeInOut(duration: 0.2), value: voiceRecording.isPanelVisible)
        .overlay(alignment: toastAlignment) {
            if let toast = ToastState.shared.content {
                HStack(alignment: toast.body == nil ? .center : .top, spacing: UIMetrics.spacing3) {
                    Image(systemName: "checkmark.circle.fill")
                        .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                        .foregroundStyle(MuxyTheme.diffAddFg)
                    VStack(alignment: .leading, spacing: UIMetrics.spacing1) {
                        Text(toast.title)
                            .font(.system(size: UIMetrics.fontBody, weight: .medium))
                            .foregroundStyle(MuxyTheme.fg)
                            .lineLimit(1)
                        if let body = toast.body {
                            Text(body)
                                .font(.system(size: UIMetrics.fontFootnote))
                                .foregroundStyle(MuxyTheme.fgMuted)
                                .lineLimit(2)
                        }
                    }
                    .frame(maxWidth: UIMetrics.scaled(360), alignment: .leading)
                }
                .padding(.horizontal, UIMetrics.scaled(14))
                .padding(.vertical, UIMetrics.spacing4)
                .fixedSize(horizontal: true, vertical: false)
                .background(MuxyTheme.bg, in: Capsule())
                .overlay(Capsule().stroke(MuxyTheme.border, lineWidth: 1))
                .contentShape(Capsule())
                .padding(toastEdgePadding)
                .transition(.move(edge: toastTransitionEdge).combined(with: .opacity))
                .allowsHitTesting(toast.isActionable)
                .onTapGesture {
                    ToastState.shared.performAction()
                }
                .accessibilityLabel(toast.accessibilityLabel)
                .accessibilityAddTraits(toast.isActionable ? .isButton : .isStaticText)
            }
        }
        .overlay { modalOverlayLayer }
        .overlay { ExtensionConsentOverlay() }
        .animation(.easeInOut(duration: 0.15), value: showTerminalOmnibox)
        .animation(.easeInOut(duration: 0.15), value: showProjectPicker)
        .animation(.easeInOut(duration: 0.15), value: ExtensionModalService.shared.active)
        .modifier(OverlayExitTracker(
            showTerminalOmnibox: showTerminalOmnibox,
            showProjectPicker: showProjectPicker,
            onAnimatingOut: { overlayAnimatingOut = $0 }
        ))
        .animation(.easeInOut(duration: 0.2), value: ToastState.shared.message != nil)
        .coordinateSpace(name: DragCoordinateSpace.mainWindow)
        .environment(dragCoordinator)
        .background(MainWindowShortcutInterceptor(
            isTerminalFocused: { isTerminalPaneFocused },
            isBrowserFocused: { isBrowserPaneFocused },
            onShortcut: { action in handleShortcutAction(action) },
            onCommandShortcut: { shortcut in handleCommandShortcut(shortcut) },
            onExtensionShortcut: { shortcut in handleExtensionShortcut(shortcut) },
            onMouseBack: { appState.goBack() },
            onMouseForward: { appState.goForward() }
        ))
        .background(WindowConfigurator(configVersion: ghostty.configVersion, uiScalePreset: UIScale.shared.preset))
        .background(WindowTitleUpdater(title: windowTitle))
        .ignoresSafeArea(.container, edges: .top)
        .onReceive(NotificationCenter.default.publisher(for: .openProjectPicker)) { _ in
            showProjectPicker = true
        }
        .onReceive(NotificationCenter.default.publisher(for: .openRemoteProjectPicker)) { notification in
            guard let deviceID = notification.userInfo?[OpenRemoteProjectPickerUserInfoKey.deviceID] as? UUID,
                  let device = remoteDeviceStore.device(id: deviceID)
            else { return }
            remoteProjectDevice = device
            showProjectPicker = true
        }
        .onReceive(NotificationCenter.default.publisher(for: .openExtensionDirectoryAsProject)) { notification in
            guard let path = notification.userInfo?[OpenExtensionDirectoryUserInfoKey.path] as? String else { return }
            CLIAccessor.openProjectFromPath(
                path,
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore,
                projectGroupStore: projectGroupStore
            )
        }
        .onReceive(NotificationCenter.default.publisher(for: .terminalOmnibox)) { notification in
            let launchScope = terminalOmniboxScope(from: notification)
            if showTerminalOmnibox, launchScope != terminalOmniboxLaunchScope {
                terminalOmniboxLaunchScope = launchScope
                return
            }
            terminalOmniboxLaunchScope = launchScope
            showTerminalOmnibox.toggle()
        }
        .onReceive(NotificationCenter.default.publisher(for: .toggleSidebar)) { _ in
            withAnimation(.easeInOut(duration: 0.2)) {
                sidebarExpanded.toggle()
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .toggleExtensionConsole)) { _ in
            panelHost.toggle(BuiltinPanel.extensionConsole, at: .bottom, mode: .floating)
        }
        .onReceive(NotificationCenter.default.publisher(for: .windowFullScreenDidChange)) { notification in
            isFullScreen = notification.userInfo?["isFullScreen"] as? Bool ?? false
        }
        .modifier(SidePanelNotificationListeners(
            onToggleRichInput: { toggleRichInputPanel() },
            onToggleVoiceRecording: { _ = openVoiceRecorder() }
        ))
        .onChange(of: worktreeKeysSignature) {
            pruneWorktreeStates()
            pruneVisitedWorktreeKeys()
        }
        .onChange(of: activeWorktreeSignature) {
            updateWorkspaceFileWatcher()
            recordVisitedActiveWorktree()
        }
        .onChange(of: appState.activeProjectID) {
            activateWorkspaceForActiveProject()
        }
        .task {
            updateWorkspaceFileWatcher()
            recordVisitedActiveWorktree()
        }
        .modifier(TabCloseConfirmationObserver(
            lastTab: appState.pendingLastTabClose != nil,
            runningProcess: appState.pendingProcessTabClose != nil,
            onLastTab: { presentCloseConfirmation(.lastTab) },
            onRunningProcess: { presentCloseConfirmation(.runningProcess) }
        ))
        .onChange(of: appState.pendingLayoutApply != nil) { _, isPresented in
            guard isPresented, let pending = appState.pendingLayoutApply else { return }
            presentLayoutApplyConfirmation(pending: pending)
        }
        .modifier(SentryConsentPrompter())
    }

    private var leftNavigationColumn: some View {
        VStack(spacing: 0) {
            if !isFullScreen {
                Color.clear
                    .frame(height: UIMetrics.titleBarHeight)
                    .background(WindowDragRepresentable())

                Rectangle().fill(MuxyTheme.border).frame(height: 1)
                    .accessibilityHidden(true)
            }

            Sidebar(
                expanded: sidebarExpanded,
                expandedCustomWidth: CGFloat(sidebarExpandedCustomWidth)
            )
        }
        .frame(width: leftNavigationWidth, alignment: .leading)
        .clipped()
        .background(MuxyTheme.bg)
        .overlay(alignment: .trailing) {
            if leftNavigationWidth > 0, !sidebarIsResizable {
                Rectangle().fill(MuxyTheme.border)
                    .frame(width: 1)
                    .padding(.top, leftNavigationBorderTopPadding)
                    .accessibilityHidden(true)
            }
        }
        .fixedSize(horizontal: true, vertical: false)
        .animation(.easeInOut(duration: 0.2), value: sidebarExpanded)
    }

    private var mainWorkspaceColumn: some View {
        VStack(spacing: 0) {
            mainTitleBarContent
                .frame(height: UIMetrics.titleBarHeight)
                .background(WindowDragRepresentable())
                .background(MuxyTheme.bg)

            Rectangle().fill(MuxyTheme.border).frame(height: 1)
                .background(MuxyTheme.bg)

            workspaceContent
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var mainTitleBarContent: some View {
        HStack(spacing: 0) {
            if mainTitleBarLeadingInset > 0 {
                Color.clear
                    .frame(width: mainTitleBarLeadingInset)
                    .fixedSize(horizontal: true, vertical: false)
            }

            topBarContent
        }
        .animation(.easeInOut(duration: 0.2), value: sidebarExpanded)
    }

    @ViewBuilder
    private var titleBarNavigationOverlay: some View {
        if !isFullScreen {
            Color.clear
                .frame(width: titleBarNavigationOverlayWidth, height: UIMetrics.titleBarHeight)
                .fixedSize(horizontal: true, vertical: false)
                .background(WindowDragRepresentable())
                .background(MuxyTheme.bg)
                .overlay(alignment: .trailing) {
                    HStack(spacing: 0) {
                        navigationArrows
                        if titleBarNavigationOverflowsSidebar {
                            Rectangle().fill(MuxyTheme.border).frame(width: 1)
                                .accessibilityHidden(true)
                        }
                    }
                }
                .animation(.easeInOut(duration: 0.2), value: sidebarExpanded)
        }
    }

    private var workspaceContent: some View {
        VStack(spacing: 0) {
            HStack(spacing: 0) {
                ZStack {
                    MuxyTheme.bg
                    if let project = activeProject,
                       !appState.hasTabs(for: project.id),
                       let worktree = resolvedActiveWorktree(for: project)
                    {
                        EmptyProjectPlaceholder(project: project) {
                            appState.openInitialTab(projectID: project.id, worktree: worktree)
                        }
                    } else if projectsWithTabs.isEmpty {
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

                pinnedPanelSlot(at: .right)
            }
            .overlay(alignment: .trailing) {
                floatingPanelOverlay(at: .right)
            }
            .overlay(alignment: .bottom) {
                floatingPanelOverlay(at: .bottom)
            }

            pinnedPanelSlot(at: .bottom)

            if showStatusBar {
                ProjectStatusBar(
                    activePane: activeTerminalPane,
                    activeWorktree: activeProject.flatMap { resolvedActiveWorktree(for: $0) },
                    fallbackProjectPath: activeProject.map { activeWorktreePath(for: $0) },
                    isRemoteWorkspace: activeProject.map {
                        projectGroupStore.workspaceContext(for: $0).isRemote
                    } ?? false,
                    isInteractive: activeTerminalPane != nil && !overlayAnimatingOut,
                    richInputVisible: richInputPanelVisible,
                    richInputFontSize: $richInputFontSize,
                    extensionOutputVisible: extensionConsoleBinding,
                    onTriggerExtensionCommand: { binding in
                        ExtensionStore.shared.triggerCommand(
                            ExtensionStore.CommandInvocation(
                                extensionID: binding.muxyExtension.id,
                                commandID: binding.item.command,
                                appState: appState,
                                projectStore: projectStore,
                                worktreeStore: worktreeStore,
                                projectGroupStore: projectGroupStore,
                                browserProfileStore: browserProfileStore
                            )
                        )
                    }
                )
            }
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
                showDevelopmentBadge: AppEnvironment.isDevelopment,
                openInIDEProjectPath: project.isRemote ? nil : activeWorktreePath(for: project),
                projectID: project.id,
                onSelectTab: { tabID in
                    appState.dispatch(.selectTab(projectID: project.id, areaID: area.id, tabID: tabID))
                },
                onCreateTab: {
                    appState.dispatch(.createTab(projectID: project.id, areaID: area.id))
                },
                onOpenBrowser: browserEnabled ? {
                    appState.dispatch(.createBrowserTab(
                        projectID: project.id,
                        areaID: area.id,
                        url: BrowserURL.homeURL,
                        profileID: browserProfileStore.defaultProfileID
                    ))
                } : nil,
                onCloseTab: { tabID in
                    appState.closeTab(tabID, areaID: area.id, projectID: project.id)
                },
                onCloseOtherTabs: { tabID in
                    let ids = area.tabs.filter { $0.id != tabID && !$0.isPinned }.map(\.id)
                    appState.closeTabs(ids, areaID: area.id, projectID: project.id)
                },
                onCloseTabsToLeft: { tabID in
                    guard let index = area.tabs.firstIndex(where: { $0.id == tabID }) else { return }
                    let ids = area.tabs.prefix(index).filter { !$0.isPinned }.map(\.id)
                    appState.closeTabs(ids, areaID: area.id, projectID: project.id)
                },
                onCloseTabsToRight: { tabID in
                    guard let index = area.tabs.firstIndex(where: { $0.id == tabID }) else { return }
                    let ids = area.tabs.suffix(from: index + 1).filter { !$0.isPinned }.map(\.id)
                    appState.closeTabs(ids, areaID: area.id, projectID: project.id)
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
                        if let version = UpdateService.shared.availableUpdateVersion {
                            UpdateBadge(version: version) {
                                UpdateService.shared.checkForUpdates()
                            }
                            .padding(.trailing, UIMetrics.spacing2)
                        }
                        if AppEnvironment.isDevelopment {
                            devModeBadge
                                .padding(.trailing, UIMetrics.spacing3)
                        }
                        if let project = activeProject {
                            if !project.isRemote {
                                OpenInIDEControl(projectPath: activeWorktreePath(for: project))
                            }
                            LayoutPickerMenu(projectID: project.id)
                        }
                        ExtensionTopbarItems()
                    }
                    .padding(.trailing, UIMetrics.spacing2)
                }
        }
    }

    private var overlayActive: Bool {
        showTerminalOmnibox
            || showProjectPicker
            || ExtensionModalService.shared.active != nil
            || overlayAnimatingOut
    }

    @ViewBuilder
    private var modalOverlayLayer: some View {
        terminalOmniboxOverlay
        projectPickerOverlay
        extensionModalOverlay
    }

    @ViewBuilder
    private var extensionModalOverlay: some View {
        if let request = ExtensionModalService.shared.active {
            ExtensionModalOverlay(
                request: request,
                onSelect: { item in ExtensionModalService.shared.select(item) },
                onDismiss: { ExtensionModalService.shared.dismiss() }
            )
            .transition(.opacity.combined(with: .scale(scale: 0.98)))
        }
    }

    @ViewBuilder
    private var terminalOmniboxOverlay: some View {
        if showTerminalOmnibox {
            TerminalOmniboxOverlay(
                projects: terminalOmniboxProjects,
                worktrees: terminalOmniboxWorktrees,
                workspaces: terminalOmniboxWorkspaces,
                openTabs: terminalOmniboxOpenTabs,
                commandShortcuts: CommandShortcutStore.shared.shortcuts,
                extensionCommands: terminalOmniboxExtensionCommands,
                activeProjectID: appState.activeProjectID,
                activeWorktreeID: appState.activeProjectID.flatMap { appState.activeWorktreeID[$0] },
                commandProjectIDs: terminalOmniboxCommandProjectIDs,
                launchScope: terminalOmniboxLaunchScope,
                onSelect: { item, scopedProjectID, scopedWorktreeID in
                    showTerminalOmnibox = false
                    handleTerminalOmniboxSelection(
                        item,
                        scopedProjectID: scopedProjectID,
                        scopedWorktreeID: scopedWorktreeID
                    )
                },
                onDismiss: { showTerminalOmnibox = false }
            )
            .transition(.opacity.combined(with: .scale(scale: 0.98)))
        }
    }

    @ViewBuilder
    private var projectPickerOverlay: some View {
        if showProjectPicker {
            ProjectPickerOverlay(
                projectPaths: projectPickerPaths,
                context: projectPickerContext,
                onConfirm: { path, createIfMissing in
                    if let device = remoteProjectDevice {
                        return RemoteDeviceProjectConfirmationService(
                            appState: appState,
                            projectStore: projectStore,
                            worktreeStore: worktreeStore,
                            projectGroupStore: projectGroupStore
                        )
                        .confirm(path: path, device: device)
                    }
                    if projectGroupStore.isRemoteWorkspaceActive {
                        return confirmRemoteProjectPath(path)
                    }
                    return ProjectOpenService.confirmProjectPathResult(
                        path,
                        appState: appState,
                        projectStore: projectStore,
                        worktreeStore: worktreeStore,
                        projectGroupStore: projectGroupStore,
                        createIfMissing: createIfMissing
                    )
                },
                onChooseFinder: {
                    ProjectOpenService.openProject(
                        appState: appState,
                        projectStore: projectStore,
                        worktreeStore: worktreeStore,
                        projectGroupStore: projectGroupStore
                    )
                },
                onDismiss: {
                    showProjectPicker = false
                    remoteProjectDevice = nil
                }
            )
            .transition(.opacity.combined(with: .scale(scale: 0.98)))
        }
    }

    private func handleTerminalOmniboxSelection(
        _ item: TerminalOmniboxItem,
        scopedProjectID: UUID?,
        scopedWorktreeID: UUID?
    ) {
        switch item {
        case let .project(project):
            _ = selectOmniboxProject(project.projectID)
        case let .worktree(worktree):
            _ = selectOmniboxProject(worktree.projectID, worktreeID: worktree.worktreeID)
        case let .workspace(workspace):
            selectOmniboxWorkspace(workspace)
        case let .openTab(tab):
            _ = selectOmniboxProject(tab.projectID, worktreeID: tab.worktreeID)
            appState.dispatch(.selectTab(projectID: tab.projectID, areaID: tab.areaID, tabID: tab.tabID))
        case let .commandShortcut(shortcut):
            guard let projectID = scopedProjectID else { return }
            _ = selectOmniboxProject(projectID, worktreeID: scopedWorktreeID)
            appState.createCommandTab(projectID: projectID, shortcut: shortcut)
        case let .extensionCommand(item):
            ExtensionStore.shared.triggerCommand(.init(
                extensionID: item.extensionID,
                commandID: item.command.id,
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore
            ))
        }
    }

    private var terminalOmniboxExtensionCommands: [ExtensionPaletteItem] {
        ExtensionStore.shared.paletteCommands().map { binding in
            ExtensionPaletteItem(
                extensionID: binding.muxyExtension.id,
                extensionName: binding.muxyExtension.displayName,
                command: binding.command
            )
        }
    }

    private func terminalOmniboxScope(from notification: Notification) -> TerminalOmniboxLaunchScope {
        guard let rawValue = notification.userInfo?["launchScope"] as? String,
              let scope = TerminalOmniboxLaunchScope(rawValue: rawValue)
        else { return .openTabs }
        return scope
    }

    private var omniboxProjects: [Project] {
        if projectGroupStore.isRemoteWorkspaceActive {
            let remoteHome = showHomeProject ? projectGroupStore.activeRemoteHomeProject.map { [$0] } ?? [] : []
            return remoteHome + projectGroupStore.displayProjects(localProjects: projectStore.storedProjects)
        }
        let sorted = ProjectSortMode.current.sorted(projectStore.storedProjects)
        return showHomeProject ? [Project.home] + sorted : sorted
    }

    private var terminalOmniboxProjects: [TerminalOmniboxProjectItem] {
        omniboxProjects.map {
            TerminalOmniboxProjectItem(projectID: $0.id, name: $0.name, path: $0.path)
        }
    }

    private var terminalOmniboxWorktrees: [TerminalOmniboxWorktreeItem] {
        let items = omniboxProjects.flatMap { project in
            worktreeStore.list(for: project.id).map { worktree in
                TerminalOmniboxWorktreeItem(
                    projectID: project.id,
                    worktreeID: worktree.id,
                    name: worktree.name,
                    path: worktree.path,
                    branch: worktree.branch,
                    isPrimary: worktree.isPrimary
                )
            }
        }
        guard orderWorktreesByMRU else { return items }
        var mruRank: [WorktreeKey: Int] = [:]
        for (index, key) in appState.worktreeMRU.enumerated() {
            mruRank[key] = index
        }
        return items.enumerated().sorted { lhs, rhs in
            let lhsRank = mruRank[WorktreeKey(projectID: lhs.element.projectID, worktreeID: lhs.element.worktreeID)] ?? Int.max
            let rhsRank = mruRank[WorktreeKey(projectID: rhs.element.projectID, worktreeID: rhs.element.worktreeID)] ?? Int.max
            if lhsRank != rhsRank { return lhsRank < rhsRank }
            return lhs.offset < rhs.offset
        }.map(\.element)
    }

    private var terminalOmniboxWorkspaces: [TerminalOmniboxWorkspaceItem] {
        let storedProjects = projectStore.storedProjects
        let allProjects = TerminalOmniboxWorkspaceItem(
            groupID: nil,
            name: "All Projects",
            projectCount: storedProjects.count
        )
        let groups = projectGroupStore.groups.map { group in
            TerminalOmniboxWorkspaceItem(
                groupID: group.id,
                name: group.name,
                projectCount: storedProjects.count { group.projectIDs.contains($0.id) }
            )
        }
        return [allProjects] + groups
    }

    private var terminalOmniboxOpenTabs: [OpenTerminalTabItem] {
        omniboxProjects.flatMap { appState.allOpenTerminalTabItems(for: $0.id) }
    }

    private var terminalOmniboxCommandProjectIDs: Set<UUID> {
        Set(omniboxProjects.compactMap { project in
            worktreeStore.preferred(for: project.id, matching: appState.activeWorktreeID[project.id]) == nil
                ? nil
                : project.id
        })
    }

    private func selectOmniboxProject(_ projectID: UUID, worktreeID: UUID? = nil) -> Bool {
        guard let project = resolveOmniboxProject(projectID) else { return false }
        if project.isRemote { worktreeStore.ensurePrimary(for: project) }
        let worktree = if let worktreeID {
            worktreeStore.list(for: project.id).first { $0.id == worktreeID }
        } else {
            worktreeStore.preferred(for: project.id, matching: appState.activeWorktreeID[project.id])
        }
        guard let worktree else { return false }
        appState.selectProject(project, worktree: worktree)
        return true
    }

    private func activateWorkspaceForActiveProject() {
        guard let projectID = appState.activeProjectID, projectID != Project.homeID else { return }
        let candidates = projectStore.projects + projectGroupStore.remoteProjects
        guard let project = candidates.first(where: { $0.id == projectID }) else { return }
        projectGroupStore.activateWorkspaceForProjectSelection(containing: project)
    }

    private func resolveOmniboxProject(_ projectID: UUID) -> Project? {
        if let project = projectStore.projects.first(where: { $0.id == projectID }) {
            return project
        }
        return omniboxProjects.first(where: { $0.id == projectID })
    }

    private func selectOmniboxWorkspace(_ workspace: TerminalOmniboxWorkspaceItem) {
        guard let groupID = workspace.groupID else {
            projectGroupStore.clearGroupSelection()
            selectFirstProjectOfActiveWorkspace()
            return
        }
        projectGroupStore.selectGroup(id: groupID)
        selectFirstProjectOfActiveWorkspace()
    }

    private func selectFirstProjectOfActiveWorkspace() {
        WorkspaceSelectionService.selectFirstProject(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )
    }

    private var projectPickerContext: WorkspaceContext {
        if let device = remoteProjectDevice {
            return .ssh(device.destination)
        }
        return projectGroupStore.activeWorkspaceContext
    }

    private var projectPickerPaths: [String] {
        if let device = remoteProjectDevice {
            return projectStore.storedProjects
                .filter { $0.remoteDeviceID == device.id }
                .map(\.path)
        }
        if projectGroupStore.isRemoteWorkspaceActive {
            return projectGroupStore.activeRemoteProjects.map(\.path)
        }
        return projectStore.projects.map(\.path)
    }

    private func confirmRemoteProjectPath(_ path: String) -> ProjectOpenConfirmationResult {
        guard let group = projectGroupStore.activeGroup, group.type == .ssh else { return .failed }
        let name = path.split(separator: "/").last.map(String.init) ?? path
        guard let remote = projectGroupStore.addRemoteProject(name: name, path: path, toGroup: group.id) else {
            return .failed
        }
        let project = remote.asProject(workspaceID: group.id, sortOrder: group.remoteProjects.count)
        worktreeStore.ensurePrimary(for: project)
        guard let primary = worktreeStore.primary(for: project.id) else { return .failed }
        appState.selectProject(project, worktree: primary)
        return .success
    }

    private var toastPosition: ToastPosition {
        NotificationSettings.toastPosition(rawValue: toastPositionRaw)
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

    private var sidebarResolvedWidth: CGFloat {
        SidebarLayout.resolvedWidth(
            expanded: sidebarExpanded,
            collapsedStyle: sidebarCollapsedStyle,
            expandedStyle: sidebarExpandedStyle,
            expandedCustomWidth: CGFloat(sidebarExpandedCustomWidth)
        )
    }

    private var sidebarIsResizable: Bool {
        SidebarLayout.isWide(expanded: sidebarExpanded, expandedStyle: sidebarExpandedStyle)
    }

    private var leftNavigationWidth: CGFloat {
        MainWindowLayout.leftNavigationWidth(sidebarWidth: sidebarResolvedWidth)
    }

    private var titleBarNavigationOverlayWidth: CGFloat {
        MainWindowLayout.titleBarNavigationOverlayWidth(
            leftNavigationWidth: leftNavigationWidth,
            titleBarNavigationWidth: titleBarNavigationWidth,
            isFullScreen: isFullScreen
        )
    }

    private var mainTitleBarLeadingInset: CGFloat {
        MainWindowLayout.mainTitleBarLeadingInset(
            leftNavigationWidth: leftNavigationWidth,
            titleBarNavigationOverlayWidth: titleBarNavigationOverlayWidth,
            isFullScreen: isFullScreen
        )
    }

    private var titleBarNavigationOverflowsSidebar: Bool {
        titleBarNavigationOverlayWidth > leftNavigationWidth
    }

    private var leftNavigationBorderTopPadding: CGFloat {
        titleBarNavigationOverflowsSidebar ? UIMetrics.titleBarHeight + 1 : 0
    }

    private var titleBarNavigationWidth: CGFloat {
        trafficLightWidth + navigationArrowsWidth
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

    private var allActiveProjects: [Project] {
        let remoteProjects = projectGroupStore.activeRemoteProjects.enumerated().map { index, remote in
            remote.asProject(workspaceID: projectGroupStore.activeGroupID ?? remote.id, sortOrder: index)
        }
        let remoteHome = projectGroupStore.activeRemoteHomeProject.map { [$0] } ?? []
        return projectStore.projects + remoteHome + remoteProjects
    }

    private var activeProject: Project? {
        guard let pid = appState.activeProjectID else { return nil }
        return allActiveProjects.first { $0.id == pid }
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
            projectGroupStore: projectGroupStore,
            ghostty: ghostty
        )
    }

    private func mountedWorktreeKeys(for project: Project) -> [WorktreeKey] {
        var keys = visitedWorktreeKeys.filter {
            $0.projectID == project.id && appState.workspaceRoots[$0] != nil
        }
        if let activeKey = appState.activeWorktreeKey(for: project.id),
           appState.workspaceRoots[activeKey] != nil
        {
            keys.insert(activeKey)
        }
        return keys.sorted { $0.worktreeID.uuidString < $1.worktreeID.uuidString }
    }

    private func recordVisitedActiveWorktree() {
        guard let projectID = appState.activeProjectID,
              let key = appState.activeWorktreeKey(for: projectID)
        else { return }
        visitedWorktreeKeys.insert(key)
    }

    private func pruneVisitedWorktreeKeys() {
        visitedWorktreeKeys = visitedWorktreeKeys.filter { appState.workspaceRoots[$0] != nil }
    }

    private var isTerminalPaneFocused: Bool {
        guard let projectID = appState.activeProjectID else { return false }
        return appState.activeTab(for: projectID)?.content.pane != nil
    }

    private var isBrowserPaneFocused: Bool {
        guard browserEnabled,
              let projectID = appState.activeProjectID
        else { return false }
        return appState.activeTab(for: projectID)?.content.browserState != nil
    }

    private func handleShortcutAction(_ action: ShortcutAction) -> Bool {
        if action == .toggleVoiceRecording {
            return openVoiceRecorder()
        }
        return shortcutDispatcher.perform(action, activeProject: activeProject)
    }

    private func openVoiceRecorder() -> Bool {
        if voiceRecording.isPanelVisible {
            voiceRecording.cancel()
            return true
        }
        voiceRecording.present(languageIdentifier: recordingLanguage)
        return true
    }

    private func handleCommandShortcut(_ shortcut: CommandShortcut) -> Bool {
        guard let projectID = appState.activeProjectID,
              appState.workspaceRoot(for: projectID) != nil,
              !shortcut.trimmedCommand.isEmpty
        else { return false }
        appState.createCommandTab(projectID: projectID, shortcut: shortcut)
        return true
    }

    private func handleExtensionShortcut(_ shortcut: ExtensionShortcut) -> Bool {
        ExtensionStore.shared.triggerCommand(.init(
            extensionID: shortcut.extensionID,
            commandID: shortcut.commandID,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore
        ))
        return true
    }

    private var projectsWithTabs: [Project] {
        allActiveProjects.filter { appState.hasTabs(for: $0.id) }
    }

    var richInputPanelVisible: Bool { panelHost.isOpen(BuiltinPanel.richInput) }
    var showExtensionOutput: Bool { panelHost.isOpen(BuiltinPanel.extensionConsole) }

    private var extensionConsoleBinding: Binding<Bool> {
        Binding(
            get: { panelHost.isOpen(BuiltinPanel.extensionConsole) },
            set: { _ in panelHost.toggle(BuiltinPanel.extensionConsole, at: .bottom, mode: .floating) }
        )
    }

    @ViewBuilder
    func pinnedPanelSlot(at position: PanelPosition) -> some View {
        if let panelID = panelHost.pinnedPanel(at: position) {
            panelContent(for: panelID, position: position, mode: .pinned)
        }
    }

    @ViewBuilder
    func floatingPanelOverlay(at position: PanelPosition) -> some View {
        if let panelID = panelHost.floatingPanel(at: position) {
            panelContent(for: panelID, position: position, mode: .floating)
                .background(MuxyTheme.bg)
        }
    }

    @ViewBuilder
    private func panelContent(for panelID: String, position: PanelPosition, mode: PanelMode) -> some View {
        switch panelID {
        case BuiltinPanel.richInput:
            richInputPanelBody(position: position, mode: mode)
        case BuiltinPanel.extensionConsole:
            extensionConsolePanelBody(position: position, mode: mode)
        default:
            extensionPanelBody(panelID: panelID, position: position, mode: mode)
        }
    }

    @ViewBuilder
    private func richInputPanelBody(position: PanelPosition, mode: PanelMode) -> some View {
        if let richInputState = activeRichInputState, let worktreeKey = activeWorktreeKey {
            PanelContainer(
                chrome: PanelChrome(
                    iconSymbol: "keyboard",
                    title: "Rich Input",
                    trailingButtons: [richInputBroadcastButton]
                ),
                mode: mode,
                position: position,
                onClose: { closeRichInputPanel() },
                onTogglePin: { toggleRichInputFloating() },
                onTogglePosition: { toggleRichInputPosition() },
                content: {
                    RichInputSidePanel(
                        state: richInputState,
                        worktreeKey: worktreeKey,
                        onSubmit: { appendReturn, selectedText in
                            submitRichInput(richInputState, appendReturn: appendReturn, selectedText: selectedText)
                        }
                    )
                }
            )
            .modifier(PanelFrame(
                position: position,
                size: position == .bottom ? $richInputPanelHeight : $richInputPanelWidth,
                range: position == .bottom
                    ? PanelLayoutMetrics.richInputHeightRange
                    : PanelLayoutMetrics.richInputWidthRange
            ))
        }
    }

    private func extensionConsolePanelBody(position: PanelPosition, mode: PanelMode) -> some View {
        PanelContainer(
            chrome: PanelChrome(
                iconSymbol: "terminal",
                title: "Extension Output",
                hiddenControls: [.pin, .position]
            ),
            mode: mode,
            position: position,
            onClose: { panelHost.close(BuiltinPanel.extensionConsole) },
            onTogglePin: nil,
            onTogglePosition: nil,
            content: {
                ExtensionOutputPanel(
                    selectedExtensionID: Binding(
                        get: { extensionOutputSelected ?? (extensionOutputSelectedStored.isEmpty ? nil : extensionOutputSelectedStored) },
                        set: { newValue in
                            extensionOutputSelected = newValue
                            extensionOutputSelectedStored = newValue ?? ""
                        }
                    )
                )
            }
        )
        .modifier(PanelFrame(
            position: position,
            size: $extensionConsoleHeight,
            range: PanelLayoutMetrics.consoleHeightRange
        ))
    }

    @ViewBuilder
    private func extensionPanelBody(panelID: String, position: PanelPosition, mode: PanelMode) -> some View {
        if let state = ExtensionPanelRegistry.shared.state(forHostPanelID: panelID) {
            ExtensionPanelView(
                state: state,
                placement: PanelPlacement(panelID: panelID, position: position, mode: mode)
            )
            .id(panelID)
            .modifier(PanelFrame(
                position: position,
                size: position == .bottom ? $extensionPanelHeight : $extensionPanelWidth,
                range: position == .bottom
                    ? PanelLayoutMetrics.extensionHeightRange
                    : PanelLayoutMetrics.extensionWidthRange
            ))
        }
    }

    private var richInputBroadcastButton: PanelHeaderButton {
        PanelHeaderButton(
            id: "richInput.broadcast",
            icon: .symbol(richInputBroadcast
                ? "dot.radiowaves.left.and.right"
                : "antenna.radiowaves.left.and.right.slash"),
            label: richInputBroadcast
                ? "Broadcast On — Send to All Split Panes"
                : "Broadcast Off — Send to Active Pane",
            isActive: richInputBroadcast,
            action: { richInputBroadcast.toggle() }
        )
    }

    private var sidebarResizeHandle: some View {
        panelResize(
            axis: .horizontal,
            edge: .trailing,
            value: $sidebarExpandedCustomWidth,
            range: SidebarLayout.minExpandedWidth ... SidebarLayout.maxExpandedWidth
        )
        .padding(.top, leftNavigationBorderTopPadding)
    }

    private func panelResize<V: BinaryFloatingPoint>(
        axis: ResizeHandle.Axis,
        edge: PanelResizeHandle.Edge,
        value: Binding<V>,
        range: ClosedRange<CGFloat>
    ) -> some View {
        PanelResizeHandle(
            axis: axis,
            edge: edge,
            current: { CGFloat(value.wrappedValue) },
            apply: { next in
                value.wrappedValue = V(min(range.upperBound, max(range.lowerBound, next)))
            }
        )
    }

    private func updateWorkspaceFileWatcher() {
        guard let project = activeProject, !project.isRemote else {
            workspaceFileWatcher.setRoot(nil)
            return
        }
        workspaceFileWatcher.setRoot(activeWorktreePath(for: project))
    }

    private func pruneWorktreeStates() {
        let validKeys = validWorktreeKeys()
        richInputStates = richInputStates.filter { validKeys.contains($0.key) }
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
            panelHost.open(BuiltinPanel.richInput, at: richInputPosition, mode: richInputMode)
            richInputState.focusVersion += 1
            return
        }
        closeRichInputPanel()
    }

    private var richInputMode: PanelMode {
        richInputFloating ? .floating : .pinned
    }

    private func toggleRichInputFloating() {
        richInputFloating.toggle()
        guard richInputPanelVisible else { return }
        panelHost.setMode(richInputMode, for: BuiltinPanel.richInput)
    }

    private func toggleRichInputPosition() {
        richInputPosition = richInputPosition.opposite
        guard richInputPanelVisible else { return }
        panelHost.move(BuiltinPanel.richInput, to: richInputPosition)
    }

    private func closeRichInputPanel() {
        panelHost.close(BuiltinPanel.richInput)
        guard let paneID = activeRichInputPaneID,
              let view = TerminalViewRegistry.shared.existingView(for: paneID)
        else { return }
        DispatchQueue.main.async {
            view.window?.makeFirstResponder(view)
        }
    }

    private func submitRichInput(_ richInput: RichInputState, appendReturn: Bool, selectedText: String?) {
        let paneIDs = richInputBroadcast ? visibleTerminalPaneIDs() : [activeRichInputPaneID].compactMap(\.self)
        guard !paneIDs.isEmpty else { return }
        RichInputSubmitter.submit(
            richInput: richInput,
            paneIDs: paneIDs,
            appendReturn: appendReturn,
            selectedText: selectedText
        )
    }

    private func visibleTerminalPaneIDs() -> [UUID] {
        guard let project = activeProject,
              let root = appState.workspaceRoot(for: project.id)
        else { return [] }
        return root.allAreas().compactMap { $0.activeTab?.content.pane?.id }
    }

    private func activeWorktreePath(for project: Project) -> String {
        guard let key = appState.activeWorktreeKey(for: project.id) else { return project.path }
        return worktreeStore
            .worktree(projectID: project.id, worktreeID: key.worktreeID)?
            .path ?? project.path
    }

    private func validWorktreeKeys() -> Set<WorktreeKey> {
        var keys: Set<WorktreeKey> = []
        for project in projectStore.projects {
            for worktree in worktreeStore.list(for: project.id) {
                keys.insert(WorktreeKey(projectID: project.id, worktreeID: worktree.id))
            }
        }
        return keys
    }

    private var worktreeKeysSignature: [String] {
        var result: [String] = []
        for project in projectStore.projects {
            result.append(project.id.uuidString)
            for worktree in worktreeStore.list(for: project.id) {
                result.append(worktree.id.uuidString)
            }
        }
        return result
    }

    private var activeWorktreeSignature: String {
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

        alert.addButton(withTitle: "Close")
        alert.addButton(withTitle: "Cancel")
        alert.buttons[0].keyEquivalent = "\r"
        alert.buttons[1].keyEquivalent = "\u{1b}"

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

private struct TabCloseConfirmationObserver: ViewModifier {
    let lastTab: Bool
    let runningProcess: Bool
    let onLastTab: () -> Void
    let onRunningProcess: () -> Void

    func body(content: Content) -> some View {
        content
            .onChange(of: lastTab) { _, isPresented in
                guard isPresented else { return }
                onLastTab()
            }
            .onChange(of: runningProcess) { _, isPresented in
                guard isPresented else { return }
                onRunningProcess()
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
    let isTerminalFocused: () -> Bool
    let isBrowserFocused: () -> Bool
    let onShortcut: (ShortcutAction) -> Bool
    let onCommandShortcut: (CommandShortcut) -> Bool
    let onExtensionShortcut: (ExtensionShortcut) -> Bool
    let onMouseBack: () -> Void
    let onMouseForward: () -> Void

    func makeNSView(context: Context) -> ShortcutInterceptingView {
        let view = ShortcutInterceptingView()
        view.isTerminalFocused = isTerminalFocused
        view.isBrowserFocused = isBrowserFocused
        view.onShortcut = onShortcut
        view.onCommandShortcut = onCommandShortcut
        view.onExtensionShortcut = onExtensionShortcut
        view.onMouseBack = onMouseBack
        view.onMouseForward = onMouseForward
        return view
    }

    func updateNSView(_ nsView: ShortcutInterceptingView, context: Context) {
        nsView.isTerminalFocused = isTerminalFocused
        nsView.isBrowserFocused = isBrowserFocused
        nsView.onShortcut = onShortcut
        nsView.onCommandShortcut = onCommandShortcut
        nsView.onExtensionShortcut = onExtensionShortcut
        nsView.onMouseBack = onMouseBack
        nsView.onMouseForward = onMouseForward
    }
}

private final class ShortcutInterceptingView: NSView {
    var isTerminalFocused: (() -> Bool)?
    var isBrowserFocused: (() -> Bool)?
    var onShortcut: ((ShortcutAction) -> Bool)?
    var onCommandShortcut: ((CommandShortcut) -> Bool)?
    var onExtensionShortcut: ((ExtensionShortcut) -> Bool)?
    var onMouseBack: (() -> Void)?
    var onMouseForward: (() -> Void)?
    private var mouseMonitor: Any?
    private var keyMonitor: Any?

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window == nil {
            removeMouseMonitor()
            removeKeyMonitor()
        } else {
            installMouseMonitorIfNeeded()
            installKeyMonitorIfNeeded()
        }
    }

    private func handleShortcutEvent(_ event: NSEvent) -> Bool {
        let scopes = ShortcutContext.activeScopes(
            for: window,
            isTerminalFocused: isTerminalFocused?() ?? false,
            isBrowserFocused: isBrowserFocused?() ?? false
        )
        let layerWasActive = CommandShortcutStore.shared.isLayerActive
        guard layerWasActive
            || !event.modifierFlags.isDisjoint(with: [.command, .control, .option])
        else {
            return false
        }

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

        if let shortcut = ExtensionShortcutStore.shared.match(event: event, scopes: scopes) {
            if onExtensionShortcut?(shortcut) == true {
                return true
            }
        }

        return false
    }

    private func installKeyMonitorIfNeeded() {
        guard keyMonitor == nil else { return }
        keyMonitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown]) { [weak self] event in
            guard let self,
                  let window = self.window,
                  window.isKeyWindow,
                  ShortcutContext.isMainWindow(window)
            else { return event }
            return self.handleShortcutEvent(event) ? nil : event
        }
    }

    private func removeKeyMonitor() {
        guard let keyMonitor else { return }
        NSEvent.removeMonitor(keyMonitor)
        self.keyMonitor = nil
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

private struct SidePanelNotificationListeners: ViewModifier {
    let onToggleRichInput: () -> Void
    let onToggleVoiceRecording: () -> Void

    func body(content: Content) -> some View {
        content
            .onReceive(NotificationCenter.default.publisher(for: .toggleRichInput)) { _ in
                onToggleRichInput()
            }
            .onReceive(NotificationCenter.default.publisher(for: .toggleVoiceRecording)) { _ in
                onToggleVoiceRecording()
            }
    }
}

private struct SentryConsentPrompter: ViewModifier {
    @State private var hasPrompted = false

    func body(content: Content) -> some View {
        content.task {
            guard !hasPrompted, SentryService.shared.needsPrompt else { return }
            hasPrompted = true
            await presentWhenWindowReady()
        }
    }

    @MainActor
    private func presentWhenWindowReady() async {
        if let window = readyWindow() {
            present(on: window)
            return
        }
        await waitForKeyWindow()
        if let window = readyWindow() {
            present(on: window)
        }
    }

    @MainActor
    private func waitForKeyWindow() async {
        let center = NotificationCenter.default
        let holder = ObserverHolder()
        await withCheckedContinuation { (continuation: CheckedContinuation<Void, Never>) in
            holder.token = center.addObserver(
                forName: NSWindow.didBecomeKeyNotification,
                object: nil,
                queue: .main
            ) { _ in
                MainActor.assumeIsolated {
                    guard NSApp.keyWindow ?? NSApp.mainWindow != nil else { return }
                    if let token = holder.token {
                        center.removeObserver(token)
                        holder.token = nil
                        continuation.resume()
                    }
                }
            }
        }
    }

    @MainActor
    private func readyWindow() -> NSWindow? {
        guard let window = NSApp.keyWindow ?? NSApp.mainWindow else { return nil }
        return window.attachedSheet == nil ? window : nil
    }

    @MainActor
    private func present(on window: NSWindow) {
        let alert = NSAlert()
        alert.messageText = "Help improve Muxy?"
        alert.informativeText = """
        Muxy can send anonymous crash and error reports so we can fix bugs faster. \
        No personal data, no project contents, no file paths are sent — only crash \
        details and an anonymous installation ID.

        You can change this anytime in Settings → General → Diagnostics.
        """
        alert.alertStyle = .informational
        alert.icon = NSApp.applicationIconImage
        alert.addButton(withTitle: "Allow")
        alert.addButton(withTitle: "Don't Allow")
        alert.buttons[0].keyEquivalent = "\r"
        alert.buttons[1].keyEquivalent = "\u{1b}"

        alert.beginSheetModal(for: window) { response in
            let consent: SentryConsent = response == .alertFirstButtonReturn ? .allowed : .denied
            SentryService.shared.setConsent(consent)
        }
    }
}

@MainActor
private final class ObserverHolder {
    var token: NSObjectProtocol?
}

private struct OverlayExitTracker: ViewModifier {
    let showTerminalOmnibox: Bool
    let showProjectPicker: Bool
    let onAnimatingOut: (Bool) -> Void

    func body(content: Content) -> some View {
        content
            .onChange(of: showTerminalOmnibox) { _, visible in trackExit(visible) }
            .onChange(of: showProjectPicker) { _, visible in trackExit(visible) }
    }

    private func trackExit(_ visible: Bool) {
        guard !visible else { return }
        onAnimatingOut(true)
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.15) {
            onAnimatingOut(false)
        }
    }
}
