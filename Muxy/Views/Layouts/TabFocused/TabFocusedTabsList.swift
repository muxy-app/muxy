import MuxyShared
import SwiftUI

struct TabFocusedTabActions: View {
    let project: Project
    let worktree: Worktree?

    @Environment(AppState.self) private var appState
    @Environment(WorktreeStore.self) private var worktreeStore
    @Environment(BrowserProfileStore.self) private var browserProfileStore
    @AppStorage(BrowserPreferences.enabledKey) private var browserEnabled = true

    private var targetKey: WorktreeKey? {
        worktree.map { WorktreeKey(projectID: project.id, worktreeID: $0.id) }
    }

    var body: some View {
        SidebarActionButton(symbol: "plus", label: L10n.string("New Terminal Tab")) {
            guard let targetKey = activatedTargetKey() else { return }
            appState.dispatch(.createTabInWorktree(key: targetKey, areaID: nil))
        }
        if browserEnabled {
            SidebarActionButton(symbol: "globe", label: L10n.string("New Browser Tab")) {
                guard let targetKey = activatedTargetKey() else { return }
                appState.dispatch(.createBrowserTabInWorktree(
                    key: targetKey,
                    areaID: nil,
                    url: BrowserURL.homeURL,
                    profileID: browserProfileStore.defaultProfileID
                ))
            }
        }
    }

    private func activatedTargetKey() -> WorktreeKey? {
        guard let targetKey else { return nil }
        TabFocusedSidebarTarget.activate(
            project: project,
            worktree: worktree,
            appState: appState,
            worktreeStore: worktreeStore
        )
        return targetKey
    }
}

struct AgentsFocusedTabActions: View {
    let project: Project
    let worktree: Worktree?
    @Binding var showingProviders: Bool

    @Environment(AppState.self) private var appState
    @Environment(ProjectGroupStore.self) private var projectGroupStore
    @Environment(WorktreeStore.self) private var worktreeStore
    @State private var hovered = false
    @State private var options: [AgentTabLaunchOption] = []
    @State private var loadingProviders = false
    @State private var providerTask: Task<Void, Never>?

    private var workspaceContext: WorkspaceContext {
        projectGroupStore.workspaceContext(for: project)
    }

    var body: some View {
        Button {
            presentProviders()
        } label: {
            Image(systemName: "plus")
                .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                .foregroundStyle(hovered ? MuxyTheme.fg : MuxyTheme.fgMuted)
                .frame(width: TabFocusedSidebarMetrics.controlSlot, height: TabFocusedSidebarMetrics.controlSlot)
                .background {
                    RoundedRectangle(cornerRadius: UIMetrics.radiusSM, style: .continuous)
                        .fill(hovered || showingProviders ? MuxyTheme.hover : Color.clear)
                }
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .popover(isPresented: $showingProviders, arrowEdge: .trailing) {
            if loadingProviders {
                AgentsFocusedProviderLoadingMenu()
            } else {
                AgentsFocusedProviderMenu(options: options) { option in
                    showingProviders = false
                    launch(option)
                }
            }
        }
        .onChange(of: showingProviders) { _, showing in
            guard !showing else { return }
            providerTask?.cancel()
            loadingProviders = false
        }
        .onDisappear { providerTask?.cancel() }
        .help(L10n.string("New Agent Tab"))
        .accessibilityLabel(L10n.string("New Agent Tab"))
    }

    private func presentProviders() {
        providerTask?.cancel()
        guard case let .ssh(destination) = workspaceContext else {
            options = AgentTabLaunchOption.resolveLocal()
            loadingProviders = false
            showingProviders = true
            return
        }

        options = []
        loadingProviders = true
        showingProviders = true
        providerTask = Task {
            do {
                let resolved = try await AgentTabLaunchOption.resolveRemote(destination: destination)
                guard !Task.isCancelled else { return }
                options = resolved
                loadingProviders = false
            } catch {
                guard !Task.isCancelled else { return }
                loadingProviders = false
                showingProviders = false
                ToastState.shared.show(
                    title: L10n.string("Could not check remote agent providers"),
                    body: error.localizedDescription
                )
            }
        }
    }

    private func launch(_ option: AgentTabLaunchOption) {
        guard let command = option.command else { return }
        AgentsFocusedTabLauncher.launch(
            request: AgentsFocusedTabLaunchRequest(
                project: project,
                worktree: worktree,
                providerID: option.provider.id,
                name: option.provider.displayName,
                command: command
            ),
            appState: appState,
            worktreeStore: worktreeStore
        )
    }
}

struct AgentsFocusedTabLaunchRequest {
    let project: Project
    let worktree: Worktree?
    let providerID: String
    let name: String
    let command: String
}

@MainActor
enum AgentsFocusedTabLauncher {
    static func launch(
        request: AgentsFocusedTabLaunchRequest,
        appState: AppState,
        worktreeStore: WorktreeStore
    ) {
        TabFocusedSidebarTarget.activate(
            project: request.project,
            worktree: request.worktree,
            appState: appState,
            worktreeStore: worktreeStore
        )
        let effects = appState.dispatchReturningEffects(.createCommandTab(CommandTabRequest(
            projectID: request.project.id,
            areaID: nil,
            name: request.name,
            command: request.command,
            closesOnCommandExit: false
        )))
        guard let tabID = effects.createdTabID,
              let key = appState.activeWorktreeKey(for: request.project.id),
              let paneID = appState.workspaceRoots[key]?.locateTab(id: tabID)?.tab.content.pane?.id
        else { return }
        DetectedAgentStore.shared.setProvisionalAgent(request.providerID, for: paneID)
    }
}

struct TabFocusedTabsList: View {
    let project: Project
    let worktree: Worktree
    let shortcutNumbers: [UUID: Int]

    @Environment(AppState.self) private var appState
    @State private var dragState = TabFocusedDragState()

    private struct AreaTab: Identifiable {
        let area: TabArea
        let tab: TerminalTab
        let relatedTabs: [TerminalTab]
        let worktree: Worktree
        var id: UUID { tab.id }
    }

    private var worktreeKey: WorktreeKey {
        WorktreeKey(projectID: project.id, worktreeID: worktree.id)
    }

    private var areaTabs: [AreaTab] {
        let allTabs = appState.workspaceRoots[worktreeKey]?.allTabs() ?? []
        return appState.topLevelTabs(for: worktreeKey).map { item in
            AreaTab(
                area: item.area,
                tab: item.tab,
                relatedTabs: [item.tab] + allTabs.filter { $0.parentTabID == item.tab.id },
                worktree: worktree
            )
        }
    }

    private var activeTabID: UUID? {
        guard appState.activeProjectID == project.id,
              appState.activeWorktreeID[project.id] == worktree.id
        else { return nil }
        return appState.activeTopLevelTabID(for: worktreeKey)
    }

    var body: some View {
        VStack(spacing: 0) {
            tabRows(areaTabs, numbers: shortcutNumbers)
        }
        .coordinateSpace(name: TabFocusedDragCoordinateSpace.list)
        .onPreferenceChange(TabFocusedRowFramePreferenceKey.self) { frames in
            guard dragState.draggedID != nil else { return }
            dragState.frames = frames
        }
    }

    private func tabRows(_ tabs: [AreaTab], numbers: [UUID: Int]) -> some View {
        ForEach(tabs) { item in
            TabFocusedTabRow(
                project: project,
                area: item.area,
                tab: item.tab,
                relatedTabs: item.relatedTabs,
                topLevelTabs: areaTabs.map { ($0.area, $0.tab) },
                active: item.tab.id == activeTabID,
                worktree: item.worktree,
                shortcutNumber: numbers[item.tab.id]
            )
            .opacity(dragState.draggedID == item.tab.id ? 0.5 : 1)
            .background {
                if dragState.draggedID != nil {
                    GeometryReader { geo in
                        Color.clear.preference(
                            key: TabFocusedRowFramePreferenceKey.self,
                            value: [item.tab.id: geo.frame(in: .named(TabFocusedDragCoordinateSpace.list))]
                        )
                    }
                }
            }
            .gesture(
                DragGesture(minimumDistance: 6, coordinateSpace: .named(TabFocusedDragCoordinateSpace.list))
                    .onChanged { value in
                        handleDragChanged(item: item, location: value.location)
                    }
                    .onEnded { _ in
                        handleDragEnded()
                    }
            )
        }
    }

    private func handleDragChanged(item: AreaTab, location: CGPoint) {
        if dragState.draggedID == nil {
            dragState.draggedID = item.tab.id
            dragState.lastReorderTargetID = nil
        }
        reorderIfNeeded(at: location)
    }

    private func handleDragEnded() {
        withAnimation(.easeInOut(duration: 0.15)) {
            dragState.draggedID = nil
            dragState.frames = [:]
            dragState.lastReorderTargetID = nil
        }
    }

    private func reorderIfNeeded(at location: CGPoint) {
        guard let draggedID = dragState.draggedID else { return }
        var hoveredTargetID: UUID?

        for (id, frame) in dragState.frames where id != draggedID {
            guard frame.contains(location) else { continue }
            hoveredTargetID = id
            guard dragState.lastReorderTargetID != id,
                  let sourceIndex = areaTabs.firstIndex(where: { $0.tab.id == draggedID }),
                  let destIndex = areaTabs.firstIndex(where: { $0.tab.id == id })
            else { return }

            dragState.lastReorderTargetID = id
            let offset = destIndex > sourceIndex ? destIndex + 1 : destIndex
            withAnimation(.easeInOut(duration: 0.15)) {
                appState.reorderTopLevelTabs(
                    for: worktreeKey,
                    fromOffsets: IndexSet(integer: sourceIndex),
                    toOffset: offset
                )
            }
            return
        }

        if hoveredTargetID == nil {
            dragState.lastReorderTargetID = nil
        }
    }
}

private struct TabFocusedDragState {
    var draggedID: UUID?
    var frames: [UUID: CGRect] = [:]
    var lastReorderTargetID: UUID?
}

private enum TabFocusedDragCoordinateSpace {
    static let list = "TabFocusedTabsList"
}

private struct TabFocusedRowFramePreferenceKey: PreferenceKey {
    static let defaultValue: [UUID: CGRect] = [:]
    static func reduce(value: inout [UUID: CGRect], nextValue: () -> [UUID: CGRect]) {
        value.merge(nextValue()) { _, new in new }
    }
}

struct TabFocusedTabRow: View {
    let project: Project
    let area: TabArea
    let tab: TerminalTab
    let relatedTabs: [TerminalTab]
    let topLevelTabs: [(area: TabArea, tab: TerminalTab)]
    let active: Bool
    var worktree: Worktree?
    var shortcutNumber: Int?

    @Environment(AppState.self) private var appState
    @Environment(WorktreeStore.self) private var worktreeStore
    @State private var modifierMonitor = ModifierKeyMonitor.shared
    @State private var progressStore = TerminalProgressStore.shared

    private var projectID: UUID { project.id }

    private var shortcutHint: KeyCombo? {
        guard let shortcutNumber,
              let action = ShortcutAction.tabAction(for: shortcutNumber)
        else { return nil }
        return modifierMonitor.hint(for: action)
    }

    @State private var notificationStore = NotificationStore.shared

    private var paneProgress: TerminalProgress? {
        relatedTabs.compactMap { $0.content.pane?.id }
            .compactMap { progressStore.progress(for: $0) }
            .first
    }

    private var tabProgress: TerminalProgress? {
        TerminalProgress.tabIndicator(progress: paneProgress, agentStatus: agentStatus)
    }

    private var hasCompletionPending: Bool {
        relatedTabs.compactMap { $0.content.pane?.id }.contains {
            progressStore.isCompletionPending(for: $0)
                || AgentStatusStore.shared.isCompletionPending(forPane: $0)
        }
    }

    private var hasUnread: Bool {
        relatedTabs.contains { notificationStore.hasUnread(tabID: $0.id) }
    }

    private var isIdle: Bool {
        let panes = relatedTabs.compactMap(\.content.pane)
        return !panes.isEmpty && panes.allSatisfy(\.isOffline)
    }

    private var agentStatus: AgentStatus? {
        let statuses = relatedTabs.compactMap { tab in
            AgentStatusStore.shared.status(forPane: tab.content.pane?.id)
        }
        return statuses.contains(.waiting) ? .waiting : statuses.first
    }

    private var statusDotColor: Color? {
        if agentStatus == .waiting {
            return MuxyTheme.warning
        }
        if !active, hasUnread || hasCompletionPending {
            return MuxyTheme.accent
        }
        return nil
    }

    @State private var hovered = false
    @State private var isRenaming = false
    @State private var renameText = ""
    @State private var showColorPicker = false
    @State private var completionFlashOn = false
    @State private var flashTask: Task<Void, any Error>?
    @FocusState private var renameFieldFocused: Bool

    private var tabColor: Color? {
        ProjectIconColor.color(for: tab.colorID)
    }

    private var rowBackground: AnyShapeStyle {
        if active {
            return AnyShapeStyle(MuxyTheme.surface)
        }
        if hovered {
            return AnyShapeStyle(MuxyTheme.hover)
        }
        return AnyShapeStyle(Color.clear)
    }

    private var rowRailColor: Color? {
        guard !active else { return nil }
        return tabColor
    }

    private var currentIndex: Int? {
        topLevelTabs.firstIndex(where: { $0.tab.id == tab.id })
    }

    private var closableOthersCount: Int {
        topLevelTabs.count(where: { $0.tab.id != tab.id && !$0.tab.isPinned })
    }

    private var closableLeftCount: Int {
        guard let currentIndex else { return 0 }
        return topLevelTabs.prefix(currentIndex).count(where: { !$0.tab.isPinned })
    }

    private var closableRightCount: Int {
        guard let currentIndex else { return 0 }
        return topLevelTabs.suffix(from: currentIndex + 1).count(where: { !$0.tab.isPinned })
    }

    private var hasClosableSiblings: Bool {
        closableOthersCount > 0 || closableLeftCount > 0 || closableRightCount > 0
    }

    private var isTopLevel: Bool {
        tab.parentTabID == nil
    }

    private var closeButtonVisible: Bool {
        guard !tab.isPinned else { return false }
        return hovered
    }

    var body: some View {
        HStack(spacing: UIMetrics.spacing3) {
            leadingIcon
                .frame(width: UIMetrics.iconMD, height: UIMetrics.iconMD)
                .foregroundStyle(active ? MuxyTheme.fg : MuxyTheme.fgMuted)

            if isRenaming {
                TextField("", text: $renameText)
                    .textFieldStyle(.plain)
                    .font(.system(size: UIMetrics.fontHeadline))
                    .foregroundStyle(MuxyTheme.fg)
                    .focused($renameFieldFocused)
                    .onSubmit { commitRename() }
                    .onExitCommand { cancelRename() }
                    .onChange(of: renameFieldFocused) { _, focused in
                        if !focused, isRenaming {
                            commitRename()
                        }
                    }
            } else {
                Text(tab.localizedTitle)
                    .font(.system(size: UIMetrics.fontHeadline))
                    .foregroundStyle(active ? MuxyTheme.fg : MuxyTheme.fgMuted)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }

            Spacer(minLength: UIMetrics.spacing2)

            trailingAccessory
        }
        .padding(.leading, TabFocusedSidebarMetrics.tabContentLeadingInset)
        .padding(.trailing, TabFocusedSidebarMetrics.rowHorizontalInset)
        .frame(minHeight: TabFocusedSidebarMetrics.rowHeight)
        .background {
            RoundedRectangle(cornerRadius: TabFocusedSidebarMetrics.rowCornerRadius, style: .continuous)
                .fill(rowBackground)
        }
        .overlay {
            RoundedRectangle(cornerRadius: TabFocusedSidebarMetrics.rowCornerRadius, style: .continuous)
                .fill(MuxyTheme.accent)
                .opacity(completionFlashOn ? 0.18 : 0)
                .allowsHitTesting(false)
                .accessibilityHidden(true)
        }
        .overlay(alignment: .leading) {
            if let rowRailColor {
                RoundedRectangle(cornerRadius: TabFocusedSidebarMetrics.activeRailWidth / 2, style: .continuous)
                    .fill(rowRailColor)
                    .frame(width: TabFocusedSidebarMetrics.activeRailWidth)
                    .padding(.vertical, UIMetrics.spacing2)
                    .accessibilityHidden(true)
            }
        }
        .padding(.horizontal, TabFocusedSidebarMetrics.rowOuterInset)
        .padding(.vertical, TabFocusedSidebarMetrics.rowVerticalPadding)
        .contentShape(RoundedRectangle(cornerRadius: TabFocusedSidebarMetrics.rowCornerRadius, style: .continuous))
        .onHover { hovered = $0 }
        .onTapGesture { select() }
        .onChange(of: hasCompletionPending) { _, pending in
            guard pending else { return }
            triggerCompletionFlash()
        }
        .onDisappear { flashTask?.cancel() }
        .overlay {
            if !tab.isPinned {
                MiddleClickView { close() }
                    .accessibilityHidden(true)
            }
        }
        .overlay {
            DoubleClickView { startRename() }
                .accessibilityHidden(true)
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(tab.localizedTitle)
        .accessibilityAddTraits(active ? [.isButton, .isSelected] : .isButton)
        .contextMenu { contextMenu }
        .popover(isPresented: $showColorPicker, arrowEdge: .trailing) {
            ProjectIconColorPicker(title: "Tab Color", selectedID: tab.colorID) { id in
                area.setColorID(tab.id, colorID: id)
                appState.saveWorkspaces()
                showColorPicker = false
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .renameActiveTab)) { _ in
            guard active else { return }
            startRename()
        }
    }

    @ViewBuilder
    private var contextMenu: some View {
        if isTopLevel {
            Button(L10n.string("New Tab to the Left")) {
                appState.dispatch(
                    .createTabAdjacent(projectID: projectID, areaID: area.id, tabID: tab.id, side: .left)
                )
            }
            Button(L10n.string("New Tab to the Right")) {
                appState.dispatch(
                    .createTabAdjacent(projectID: projectID, areaID: area.id, tabID: tab.id, side: .right)
                )
            }
            Divider()
        }
        Button(L10n.string("Rename Tab")) { startRename() }
        if tab.customTitle != nil {
            Button(L10n.string("Reset Title")) {
                area.setCustomTitle(tab.id, title: nil)
                appState.saveWorkspaces()
            }
        }
        Button(L10n.string("Set Tab Color…")) { showColorPicker = true }
        if tab.colorID != nil {
            Button(L10n.string("Reset Tab Color")) {
                area.setColorID(tab.id, colorID: nil)
                appState.saveWorkspaces()
            }
        }
        if isTopLevel {
            Divider()
            Button(
                tab.isPinned
                    ? L10n.string("Unpin Tab")
                    : L10n.string("Pin Tab")
            ) {
                guard let worktree else { return }
                appState.togglePinTopLevelTab(
                    tab.id,
                    for: WorktreeKey(projectID: projectID, worktreeID: worktree.id)
                )
            }
        }
        if !tab.isPinned || isTopLevel && hasClosableSiblings {
            Divider()
            if !tab.isPinned {
                Button(L10n.string("Close Tab")) { close() }
            }
            if isTopLevel {
                Button(L10n.string("Close Other Tabs")) { closeOthers() }
                    .disabled(closableOthersCount == 0)
                Button(L10n.string("Close Tabs to the Left")) { closeLeft() }
                    .disabled(closableLeftCount == 0)
                Button(L10n.string("Close Tabs to the Right")) { closeRight() }
                    .disabled(closableRightCount == 0)
            }
        }
    }

    private var trailingAccessory: some View {
        trailingContent
            .frame(width: TabFocusedSidebarMetrics.controlSlot, height: TabFocusedSidebarMetrics.controlSlot)
    }

    @ViewBuilder
    private var trailingContent: some View {
        if !tab.isPinned, closeButtonVisible {
            Image(systemName: "xmark")
                .font(.system(size: UIMetrics.fontCaption, weight: .bold))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: UIMetrics.iconMD, height: UIMetrics.iconMD)
                .contentShape(Rectangle())
                .onTapGesture { close() }
                .accessibilityLabel(L10n.string("Close Tab"))
                .accessibilityAddTraits(.isButton)
        } else {
            statusAccessory
        }
    }

    @ViewBuilder
    private var statusAccessory: some View {
        if tab.isPinned {
            Image(systemName: "pin.fill")
                .font(.system(size: UIMetrics.fontXS, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: UIMetrics.iconMD, height: UIMetrics.iconMD)
        } else if let shortcutNumber, let combo = shortcutHint {
            ShortcutIconBadge(number: shortcutNumber, size: UIMetrics.iconMD, combo: combo)
                .frame(width: UIMetrics.iconMD, height: UIMetrics.iconMD)
        } else if let progress = tabProgress {
            TerminalProgressCircle(progress: progress)
                .frame(width: UIMetrics.iconSM, height: UIMetrics.iconSM)
                .transition(.opacity)
        } else if let dotColor = statusDotColor {
            Circle()
                .fill(dotColor)
                .frame(width: UIMetrics.scaled(7), height: UIMetrics.scaled(7))
                .frame(width: UIMetrics.iconMD, height: UIMetrics.iconMD)
        } else if isIdle, !active {
            Image(systemName: "moon.zzz")
                .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: UIMetrics.iconMD, height: UIMetrics.iconMD)
                .help(L10n.string("Idle — terminal freed to save memory. Reopens when selected."))
        } else {
            Color.clear
                .frame(width: UIMetrics.iconMD, height: UIMetrics.iconMD)
        }
    }

    @ViewBuilder
    private var leadingIcon: some View {
        switch tab.kind {
        case .terminal:
            let agentIconName = relatedTabs.compactMap {
                DetectedAgentStore.shared.iconName(forPane: $0.content.pane?.id)
            }.first
            if let agentIconName {
                ProviderIconView(iconName: agentIconName, size: UIMetrics.iconMD)
            } else {
                Image(systemName: "terminal")
                    .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
            }
        case .browser:
            if let favicon = tab.content.browserState?.faviconImage {
                Image(nsImage: favicon)
                    .resizable()
                    .interpolation(.high)
                    .frame(width: UIMetrics.iconMD, height: UIMetrics.iconMD)
            } else {
                Image(systemName: "globe")
                    .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
            }
        case .extensionWebView:
            extensionIcon
        }
    }

    @ViewBuilder
    private var extensionIcon: some View {
        if let customIcon = tab.content.extensionState?.customIcon,
           let extensionID = tab.content.extensionState?.extensionID,
           let muxyExtension = ExtensionStore.shared.loadedExtension(id: extensionID)
        {
            ExtensionIconView(icon: customIcon, muxyExtension: muxyExtension, size: 12)
        } else {
            Image(systemName: "puzzlepiece.extension")
                .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
        }
    }

    private func triggerCompletionFlash() {
        flashTask?.cancel()
        withAnimation(.easeIn(duration: 0.15)) {
            completionFlashOn = true
        }
        if active {
            for paneID in relatedTabs.compactMap({ $0.content.pane?.id }) {
                progressStore.clearCompletion(for: paneID)
                AgentStatusStore.shared.clearCompletion(for: paneID)
            }
        }
        flashTask = Task { @MainActor in
            try await Task.sleep(for: .milliseconds(450))
            withAnimation(.easeOut(duration: 0.4)) {
                completionFlashOn = false
            }
        }
    }

    private func select() {
        activateProjectIfNeeded()
        if let worktree, appState.activeWorktreeID[projectID] != worktree.id {
            appState.selectWorktree(projectID: projectID, worktree: worktree)
        }
        guard !active else { return }
        appState.dispatch(.selectTab(projectID: projectID, areaID: area.id, tabID: tab.id))
    }

    private func activateProjectIfNeeded() {
        guard appState.activeProjectID != projectID else { return }
        worktreeStore.ensurePrimary(for: project)
        let target = worktree ?? worktreeStore.preferred(
            for: projectID,
            matching: appState.activeWorktreeID[projectID]
        )
        guard let target else { return }
        appState.selectProject(project, worktree: target)
    }

    private var tabKey: WorktreeKey {
        WorktreeKey(projectID: projectID, worktreeID: worktree?.id ?? projectID)
    }

    private func close() {
        appState.closeTab(tab.id, areaID: area.id, key: tabKey)
    }

    private func closeOthers() {
        for item in topLevelTabs where item.tab.id != tab.id && !item.tab.isPinned {
            appState.closeTab(item.tab.id, areaID: item.area.id, key: tabKey)
        }
    }

    private func closeLeft() {
        guard let currentIndex else { return }
        for item in topLevelTabs.prefix(currentIndex) where !item.tab.isPinned {
            appState.closeTab(item.tab.id, areaID: item.area.id, key: tabKey)
        }
    }

    private func closeRight() {
        guard let currentIndex else { return }
        for item in topLevelTabs.suffix(from: currentIndex + 1) where !item.tab.isPinned {
            appState.closeTab(item.tab.id, areaID: item.area.id, key: tabKey)
        }
    }

    private func startRename() {
        renameText = tab.localizedTitle
        isRenaming = true
        renameFieldFocused = true
    }

    private func commitRename() {
        switch TerminalTabRename.resolve(
            input: renameText,
            displayedTitle: tab.localizedTitle,
            existingCustomTitle: tab.customTitle
        ) {
        case .unchanged:
            break
        case let .update(customTitle):
            area.setCustomTitle(tab.id, title: customTitle)
            appState.saveWorkspaces()
        }
        isRenaming = false
    }

    private func cancelRename() {
        isRenaming = false
    }
}
