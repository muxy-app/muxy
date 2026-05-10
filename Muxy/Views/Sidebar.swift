import SwiftUI

@MainActor
enum SidebarLayout {
    static var collapsedWidth: CGFloat { UIMetrics.sidebarCollapsedWidth }
    static var expandedWidth: CGFloat { UIMetrics.sidebarExpandedWidth }
    static var width: CGFloat { UIMetrics.sidebarCollapsedWidth }

    static func resolvedWidth(
        expanded: Bool,
        collapsedStyle: SidebarCollapsedStyle,
        expandedStyle: SidebarExpandedStyle
    ) -> CGFloat {
        if expanded {
            return expandedStyle == .wide ? expandedWidth : collapsedWidth
        }
        return collapsedStyle == .hidden ? 0 : collapsedWidth
    }

    static func isWide(expanded: Bool, expandedStyle: SidebarExpandedStyle) -> Bool {
        expanded && expandedStyle == .wide
    }

    static func isHidden(expanded: Bool, collapsedStyle: SidebarCollapsedStyle) -> Bool {
        !expanded && collapsedStyle == .hidden
    }
}

struct Sidebar: View {
    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore
    @State private var dragState = ProjectDragState()
    let expanded: Bool
    @AppStorage(SidebarCollapsedStyle.storageKey) private var collapsedStyleRaw = SidebarCollapsedStyle.defaultValue.rawValue
    @AppStorage(SidebarExpandedStyle.storageKey) private var expandedStyleRaw = SidebarExpandedStyle.defaultValue.rawValue

    private var collapsedStyle: SidebarCollapsedStyle {
        SidebarCollapsedStyle(rawValue: collapsedStyleRaw) ?? .defaultValue
    }

    private var expandedStyle: SidebarExpandedStyle {
        SidebarExpandedStyle(rawValue: expandedStyleRaw) ?? .defaultValue
    }

    private var isWide: Bool {
        SidebarLayout.isWide(expanded: expanded, expandedStyle: expandedStyle)
    }

    private var isHidden: Bool {
        SidebarLayout.isHidden(expanded: expanded, collapsedStyle: collapsedStyle)
    }

    var body: some View {
        VStack(spacing: 0) {
            projectList
                .frame(minHeight: 0, maxHeight: .infinity, alignment: .top)
                .clipped()

            SidebarFooter(isWide: isWide, sidebarExpanded: expanded)
                .fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxHeight: .infinity, alignment: .bottom)
        .frame(width: isHidden ? 0 : (isWide ? SidebarLayout.expandedWidth : SidebarLayout.collapsedWidth))
        .opacity(isHidden ? 0 : 1)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Sidebar")
    }

    private var addButton: some View {
        AddProjectButton(expanded: isWide) {
            ProjectOpenService.openProject(
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore
            )
        }
        .help(shortcutTooltip("Add Project", for: .openProject))
    }

    private var projectList: some View {
        ScrollView(.vertical, showsIndicators: false) {
            LazyVStack(spacing: UIMetrics.spacing2) {
                ForEach(Array(projectStore.projects.enumerated()), id: \.element.id) { index, project in
                    Group {
                        if isWide {
                            ExpandedProjectRow(
                                project: project,
                                shortcutIndex: index < 9 ? index + 1 : nil,
                                isAnyDragging: dragState.draggedID != nil,
                                onSelect: { select(project) },
                                onRemove: { remove(project) },
                                onRename: { projectStore.rename(id: project.id, to: $0) },
                                onSetLogo: { projectStore.setLogo(id: project.id, to: $0) },
                                onSetIconColor: { projectStore.setIconColor(id: project.id, to: $0) }
                            )
                        } else {
                            ProjectRow(
                                project: project,
                                shortcutIndex: index < 9 ? index + 1 : nil,
                                isAnyDragging: dragState.draggedID != nil,
                                onSelect: { select(project) },
                                onRemove: { remove(project) },
                                onRename: { projectStore.rename(id: project.id, to: $0) },
                                onSetLogo: { projectStore.setLogo(id: project.id, to: $0) },
                                onSetIconColor: { projectStore.setIconColor(id: project.id, to: $0) }
                            )
                        }
                    }
                    .background {
                        if dragState.draggedID != nil {
                            GeometryReader { geo in
                                Color.clear.preference(
                                    key: UUIDFramePreferenceKey<SidebarFrameTag>.self,
                                    value: [project.id: geo.frame(in: .named("sidebar"))]
                                )
                            }
                        }
                    }
                    .gesture(projectDragGesture(for: project))
                }
                addButton
            }
            .padding(.horizontal, isWide ? UIMetrics.spacing3 : UIMetrics.spacing4)
            .padding(.vertical, UIMetrics.spacing2)
            .onPreferenceChange(UUIDFramePreferenceKey<SidebarFrameTag>.self) { frames in
                guard dragState.draggedID != nil else { return }
                dragState.frames = frames
            }
        }
        .coordinateSpace(name: "sidebar")
    }

    private func shortcutTooltip(_ name: String, for action: ShortcutAction) -> String {
        "\(name) (\(KeyBindingStore.shared.combo(for: action).displayString))"
    }

    private func projectDragGesture(for project: Project) -> some Gesture {
        DragGesture(minimumDistance: 6, coordinateSpace: .named("sidebar"))
            .onChanged { value in
                if dragState.draggedID == nil {
                    dragState.draggedID = project.id
                    dragState.lastReorderTargetID = nil
                }
                reorderIfNeeded(at: value.location)
            }
            .onEnded { _ in
                withAnimation(.easeInOut(duration: 0.15)) {
                    dragState.draggedID = nil
                    dragState.frames = [:]
                    dragState.lastReorderTargetID = nil
                }
            }
    }

    private func select(_ project: Project) {
        worktreeStore.ensurePrimary(for: project)
        guard let worktree = worktreeStore.preferred(
            for: project.id,
            matching: appState.activeWorktreeID[project.id]
        )
        else { return }
        appState.selectProject(project, worktree: worktree)
    }

    private func remove(_ project: Project) {
        let capturedProject = project
        let knownWorktrees = worktreeStore.list(for: project.id)
        Task.detached {
            await WorktreeStore.cleanupOnDisk(for: capturedProject, knownWorktrees: knownWorktrees)
        }
        appState.removeProject(project.id)
        projectStore.remove(id: project.id)
        worktreeStore.removeProject(project.id)
    }

    private func reorderIfNeeded(at location: CGPoint) {
        guard let draggedID = dragState.draggedID else { return }
        var hoveredTargetID: UUID?

        for (id, frame) in dragState.frames where id != draggedID {
            guard frame.contains(location) else { continue }
            hoveredTargetID = id
            guard dragState.lastReorderTargetID != id else { return }

            guard let sourceIndex = projectStore.projects.firstIndex(where: { $0.id == draggedID }),
                  let destIndex = projectStore.projects.firstIndex(where: { $0.id == id })
            else { return }

            dragState.lastReorderTargetID = id
            let offset = destIndex > sourceIndex ? destIndex + 1 : destIndex
            withAnimation(.easeInOut(duration: 0.15)) {
                projectStore.reorder(
                    fromOffsets: IndexSet(integer: sourceIndex), toOffset: offset
                )
            }
            return
        }

        if hoveredTargetID == nil {
            dragState.lastReorderTargetID = nil
        }
    }
}

private struct ProjectDragState {
    var draggedID: UUID?
    var frames: [UUID: CGRect] = [:]
    var lastReorderTargetID: UUID?
}

private struct AddProjectButton: View {
    var expanded: Bool = false
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            if expanded {
                expandedLayout
            } else {
                collapsedLayout
            }
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .accessibilityLabel("Add Project")
    }

    private var collapsedLayout: some View {
        ZStack {
            RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
                .fill(MuxyTheme.hover)
            Image(systemName: "plus")
                .font(.system(size: UIMetrics.fontEmphasis, weight: .bold))
                .foregroundStyle(hovered ? MuxyTheme.accent : MuxyTheme.fgMuted)
        }
        .frame(width: UIMetrics.iconXXL, height: UIMetrics.iconXXL)
        .padding(UIMetrics.scaled(3))
    }

    private var expandedLayout: some View {
        HStack(spacing: UIMetrics.spacing4) {
            ZStack {
                RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
                    .fill(MuxyTheme.surface)
                Image(systemName: "plus")
                    .font(.system(size: UIMetrics.fontEmphasis, weight: .bold))
                    .foregroundStyle(hovered ? MuxyTheme.accent : MuxyTheme.fgMuted)
            }
            .frame(width: UIMetrics.iconXXL, height: UIMetrics.iconXXL)

            Text("Add Project")
                .font(.system(size: UIMetrics.fontBody, weight: .medium))
                .foregroundStyle(hovered ? MuxyTheme.accent : MuxyTheme.fgMuted)
                .lineLimit(1)
            Spacer()
        }
        .padding(UIMetrics.spacing2)
        .background(hovered ? MuxyTheme.hover : Color.clear, in: RoundedRectangle(cornerRadius: UIMetrics.radiusLG))
    }
}

struct SidebarFooter: View {
    var isWide = false
    var sidebarExpanded = false
    @AppStorage(AIUsageSettingsStore.usageEnabledKey) private var usageEnabled = false
    @AppStorage(AIUsageSettingsStore.usageDisplayModeKey) private var usageDisplayModeRaw = AIUsageSettingsStore.defaultUsageDisplayMode
        .rawValue
    @AppStorage(AIUsageSettingsStore.sidebarPreviewProviderIDKey) private var pinnedPreviewProviderID: String = ""
    @State private var showThemePicker = false
    @State private var showNotifications = false
    @State private var showAIUsagePopover = false
    private let usageService = AIUsageService.shared

    private var usageDisplayMode: AIUsageDisplayMode {
        AIUsageDisplayMode(rawValue: usageDisplayModeRaw) ?? AIUsageSettingsStore.defaultUsageDisplayMode
    }

    private let usageRefreshTimer = Timer.publish(every: 60, on: .main, in: .common).autoconnect()

    private var notificationStore: NotificationStore { NotificationStore.shared }

    var body: some View {
        VStack(spacing: 0) {
            if isWide {
                expandedFooter
            } else {
                collapsedFooter
            }
        }
        .task {
            await usageService.refreshIfNeeded()
        }
        .onReceive(usageRefreshTimer) { _ in
            Task {
                await usageService.refreshIfNeeded()
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .toggleThemePicker)) { _ in
            showThemePicker.toggle()
        }
        .onReceive(NotificationCenter.default.publisher(for: .toggleNotificationPanel)) { _ in
            showNotifications.toggle()
        }
        .onReceive(NotificationCenter.default.publisher(for: .toggleAIUsage)) { _ in
            guard usageEnabled else { return }
            showAIUsagePopover.toggle()
        }
        .onChange(of: usageEnabled) { _, enabled in
            if !enabled {
                showAIUsagePopover = false
            }
        }
    }

    private func postToggleSidebar() {
        NotificationCenter.default.post(name: .toggleSidebar, object: nil)
    }

    private var sidebarToggleLabel: String {
        sidebarExpanded ? "Collapse Sidebar" : "Expand Sidebar"
    }

    private var sidebarToggleIcon: String {
        "sidebar.left"
    }

    private var notificationBellIcon: String {
        notificationStore.unreadCount > 0 ? "bell.badge" : "bell"
    }

    private var previewProviderDisplay: (percent: Int, iconName: String)? {
        guard let selection = usageService.previewSelection(pinnedRawValue: pinnedPreviewProviderID),
              case .available = selection.snapshot.state
        else { return nil }

        let snapshot = selection.snapshot
        let rowPercent = selection.row?.percent
        let usedPercent = max(0, min(100, rowPercent ?? snapshot.rows.compactMap(\.percent).max() ?? 0))
        let displayPercent: Double = switch usageDisplayMode {
        case .used:
            usedPercent
        case .remaining:
            max(0, min(100, 100 - usedPercent))
        }

        return (Int(displayPercent.rounded()), snapshot.providerIconName)
    }

    private var previewProviderPercentLabel: String? {
        guard let display = previewProviderDisplay else { return nil }
        return "\(max(0, min(100, display.percent)))%"
    }

    private var aiUsageButton: some View {
        AIUsagePreviewButton(
            display: previewProviderDisplay,
            percentLabel: previewProviderPercentLabel,
            expanded: isWide,
            onTap: { showAIUsagePopover.toggle() }
        )
        .popover(isPresented: $showAIUsagePopover) {
            AIUsagePanel(
                snapshots: usageService.snapshots,
                isRefreshing: usageService.isRefreshing,
                lastRefreshDate: usageService.lastRefreshDate,
                onRefresh: refreshUsage
            )
        }
        .help("AI Usage (\(KeyBindingStore.shared.combo(for: .toggleAIUsage).displayString))")
    }

    private var collapsedFooter: some View {
        VStack(spacing: UIMetrics.spacing2) {
            if usageEnabled {
                aiUsageButton
            }
            IconButton(symbol: notificationBellIcon, accessibilityLabel: "Notifications") { showNotifications.toggle() }
                .help("Notifications")
                .popover(isPresented: $showNotifications) {
                    NotificationPanel(onDismiss: { showNotifications = false })
                }
            IconButton(symbol: "paintpalette", accessibilityLabel: "Theme Picker") { showThemePicker.toggle() }
                .help("Theme Picker (\(KeyBindingStore.shared.combo(for: .toggleThemePicker).displayString))")
                .popover(isPresented: $showThemePicker) { ThemePicker(mode: .sidebar) }
            IconButton(symbol: sidebarToggleIcon, accessibilityLabel: sidebarToggleLabel) { postToggleSidebar() }
                .help("\(sidebarToggleLabel) (\(KeyBindingStore.shared.combo(for: .toggleSidebar).displayString))")
        }
        .padding(.bottom, UIMetrics.spacing4)
    }

    private var expandedFooter: some View {
        HStack(spacing: UIMetrics.spacing2) {
            IconButton(symbol: sidebarToggleIcon, accessibilityLabel: sidebarToggleLabel) { postToggleSidebar() }
                .help("\(sidebarToggleLabel) (\(KeyBindingStore.shared.combo(for: .toggleSidebar).displayString))")

            Spacer()

            if usageEnabled {
                aiUsageButton
            }
            IconButton(symbol: notificationBellIcon, accessibilityLabel: "Notifications") { showNotifications.toggle() }
                .help("Notifications")
                .popover(isPresented: $showNotifications) {
                    NotificationPanel(onDismiss: { showNotifications = false })
                }
            IconButton(symbol: "paintpalette", accessibilityLabel: "Theme Picker") { showThemePicker.toggle() }
                .help("Theme Picker (\(KeyBindingStore.shared.combo(for: .toggleThemePicker).displayString))")
                .popover(isPresented: $showThemePicker) { ThemePicker(mode: .sidebar) }
        }
        .padding(.horizontal, UIMetrics.spacing5)
        .padding(.bottom, UIMetrics.spacing4)
    }

    private func refreshUsage() {
        Task {
            await usageService.refresh(force: true)
        }
    }
}
