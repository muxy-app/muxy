import MuxyShared
import SwiftUI

struct TabFocusedTabActions: View {
    let project: Project
    let worktree: Worktree?

    @Environment(AppState.self) private var appState
    @Environment(WorktreeStore.self) private var worktreeStore
    @Environment(BrowserProfileStore.self) private var browserProfileStore
    @AppStorage(BrowserPreferences.enabledKey) private var browserEnabled = true

    var body: some View {
        SidebarActionButton(symbol: "plus", label: "New Terminal Tab") {
            activateTarget()
            appState.createTab(projectID: project.id)
        }
        if browserEnabled {
            SidebarActionButton(symbol: "globe", label: "New Browser Tab") {
                activateTarget()
                appState.dispatch(.createBrowserTab(
                    projectID: project.id,
                    areaID: appState.focusedArea(for: project.id)?.id,
                    url: BrowserURL.homeURL,
                    profileID: browserProfileStore.defaultProfileID
                ))
            }
        }
    }

    private func activateTarget() {
        if appState.activeProjectID != project.id {
            worktreeStore.ensurePrimary(for: project)
            if let preferred = worktreeStore.preferred(for: project.id, matching: appState.activeWorktreeID[project.id]) {
                appState.selectProject(project, worktree: worktree ?? preferred)
            }
        }
        if let worktree, appState.activeWorktreeID[project.id] != worktree.id {
            appState.selectWorktree(projectID: project.id, worktree: worktree)
        }
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
        let worktree: Worktree
        var id: UUID { tab.id }
    }

    private var worktreeKey: WorktreeKey {
        WorktreeKey(projectID: project.id, worktreeID: worktree.id)
    }

    private var areaTabs: [AreaTab] {
        appState.areas(for: worktreeKey).flatMap { area in
            area.tabs.map { AreaTab(area: area, tab: $0, worktree: worktree) }
        }
    }

    private var activeTabID: UUID? {
        guard appState.activeProjectID == project.id,
              appState.activeWorktreeID[project.id] == worktree.id
        else { return nil }
        return appState.activeTab(for: project.id)?.id
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

    @ViewBuilder
    private func tabRows(_ tabs: [AreaTab], numbers: [UUID: Int]) -> some View {
        if tabs.isEmpty {
            Text("No open tabs")
                .font(.system(size: UIMetrics.fontBody))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(maxWidth: .infinity, alignment: .leading)
                .frame(minHeight: TabFocusedSidebarMetrics.rowHeight)
                .padding(
                    .leading,
                    TabFocusedSidebarMetrics.rowOuterInset + TabFocusedSidebarMetrics.tabContentLeadingInset
                )
                .padding(.trailing, TabFocusedSidebarMetrics.rowOuterInset + TabFocusedSidebarMetrics.rowHorizontalInset)
        } else {
            ForEach(tabs) { item in
                TabFocusedTabRow(
                    project: project,
                    area: item.area,
                    tab: item.tab,
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
    }

    private func handleDragChanged(item: AreaTab, location: CGPoint) {
        if dragState.draggedID == nil {
            dragState.draggedID = item.tab.id
            dragState.lastReorderTargetID = nil
        }
        reorderIfNeeded(area: item.area, at: location)
    }

    private func handleDragEnded() {
        withAnimation(.easeInOut(duration: 0.15)) {
            dragState.draggedID = nil
            dragState.frames = [:]
            dragState.lastReorderTargetID = nil
        }
    }

    private func reorderIfNeeded(area: TabArea, at location: CGPoint) {
        guard let draggedID = dragState.draggedID else { return }
        var hoveredTargetID: UUID?

        for (id, frame) in dragState.frames where id != draggedID {
            guard frame.contains(location) else { continue }
            hoveredTargetID = id
            guard dragState.lastReorderTargetID != id,
                  let sourceIndex = area.tabs.firstIndex(where: { $0.id == draggedID }),
                  let destIndex = area.tabs.firstIndex(where: { $0.id == id })
            else { return }

            dragState.lastReorderTargetID = id
            let offset = destIndex > sourceIndex ? destIndex + 1 : destIndex
            withAnimation(.easeInOut(duration: 0.15)) {
                area.reorderTab(fromOffsets: IndexSet(integer: sourceIndex), toOffset: offset)
            }
            appState.saveWorkspaces()
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

private struct TabFocusedTabRow: View {
    let project: Project
    let area: TabArea
    let tab: TerminalTab
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
        guard let paneID = tab.content.pane?.id else { return nil }
        return progressStore.progress(for: paneID)
    }

    private var tabProgress: TerminalProgress? {
        TerminalProgress.tabIndicator(progress: paneProgress, agentStatus: agentStatus)
    }

    private var hasCompletionPending: Bool {
        guard let paneID = tab.content.pane?.id else { return false }
        return progressStore.isCompletionPending(for: paneID)
    }

    private var hasUnread: Bool {
        notificationStore.hasUnread(tabID: tab.id)
    }

    private var isIdle: Bool {
        tab.content.pane?.isOffline ?? false
    }

    private var agentStatus: AgentStatus? {
        AgentStatusStore.shared.status(forPane: tab.content.pane?.id)
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
        if active { return AnyShapeStyle(MuxyTheme.surface) }
        if hovered { return AnyShapeStyle(MuxyTheme.hover) }
        return AnyShapeStyle(Color.clear)
    }

    private var rowRailColor: Color? {
        guard !active else { return nil }
        return tabColor
    }

    private var currentIndex: Int? {
        area.tabs.firstIndex(where: { $0.id == tab.id })
    }

    private var closableOthersCount: Int {
        area.tabs.count(where: { $0.id != tab.id && !$0.isPinned })
    }

    private var closableLeftCount: Int {
        guard let currentIndex else { return 0 }
        return area.tabs.prefix(currentIndex).count(where: { !$0.isPinned })
    }

    private var closableRightCount: Int {
        guard let currentIndex else { return 0 }
        return area.tabs.suffix(from: currentIndex + 1).count(where: { !$0.isPinned })
    }

    private var hasClosableSiblings: Bool {
        closableOthersCount > 0 || closableLeftCount > 0 || closableRightCount > 0
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
                        if !focused, isRenaming { commitRename() }
                    }
            } else {
                Text(tab.title)
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
        .accessibilityLabel(tab.title)
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
        Button("New Tab to the Left") {
            appState.dispatch(.createTabAdjacent(projectID: projectID, areaID: area.id, tabID: tab.id, side: .left))
        }
        Button("New Tab to the Right") {
            appState.dispatch(.createTabAdjacent(projectID: projectID, areaID: area.id, tabID: tab.id, side: .right))
        }
        Divider()
        Button("Rename Tab") { startRename() }
        if tab.customTitle != nil {
            Button("Reset Title") {
                area.setCustomTitle(tab.id, title: nil)
                appState.saveWorkspaces()
            }
        }
        Button("Set Tab Color…") { showColorPicker = true }
        if tab.colorID != nil {
            Button("Reset Tab Color") {
                area.setColorID(tab.id, colorID: nil)
                appState.saveWorkspaces()
            }
        }
        Divider()
        Button(tab.isPinned ? "Unpin Tab" : "Pin Tab") { area.togglePin(tab.id) }
        if !tab.isPinned || hasClosableSiblings {
            Divider()
            if !tab.isPinned {
                Button("Close Tab") { close() }
            }
            Button("Close Other Tabs") { closeOthers() }
                .disabled(closableOthersCount == 0)
            Button("Close Tabs to the Left") { closeLeft() }
                .disabled(closableLeftCount == 0)
            Button("Close Tabs to the Right") { closeRight() }
                .disabled(closableRightCount == 0)
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
                .accessibilityLabel("Close Tab")
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
                .help("Idle — terminal freed to save memory. Reopens when selected.")
        } else {
            Color.clear
                .frame(width: UIMetrics.iconMD, height: UIMetrics.iconMD)
        }
    }

    @ViewBuilder
    private var leadingIcon: some View {
        switch tab.kind {
        case .terminal:
            if let agentIconName = DetectedAgentStore.shared.iconName(forPane: tab.content.pane?.id) {
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
        if active, let paneID = tab.content.pane?.id {
            progressStore.clearCompletion(for: paneID)
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

    private func close() {
        appState.closeTab(tab.id, areaID: area.id, projectID: projectID)
    }

    private func closeOthers() {
        let ids = area.tabs.filter { $0.id != tab.id && !$0.isPinned }.map(\.id)
        appState.closeTabs(ids, areaID: area.id, projectID: projectID)
    }

    private func closeLeft() {
        guard let currentIndex else { return }
        let ids = area.tabs.prefix(currentIndex).filter { !$0.isPinned }.map(\.id)
        appState.closeTabs(ids, areaID: area.id, projectID: projectID)
    }

    private func closeRight() {
        guard let currentIndex else { return }
        let ids = area.tabs.suffix(from: currentIndex + 1).filter { !$0.isPinned }.map(\.id)
        appState.closeTabs(ids, areaID: area.id, projectID: projectID)
    }

    private func startRename() {
        renameText = tab.title
        isRenaming = true
        renameFieldFocused = true
    }

    private func commitRename() {
        let trimmed = renameText.trimmingCharacters(in: .whitespaces)
        area.setCustomTitle(tab.id, title: trimmed.isEmpty ? nil : trimmed)
        appState.saveWorkspaces()
        isRenaming = false
    }

    private func cancelRename() {
        isRenaming = false
    }
}
