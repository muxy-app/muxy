import AppKit
import SwiftUI

@MainActor
enum SidebarLayout {
    static var collapsedWidth: CGFloat { UIMetrics.sidebarCollapsedWidth }
    static var expandedWidth: CGFloat { UIMetrics.sidebarExpandedWidth }
    static var minExpandedWidth: CGFloat { UIMetrics.sidebarExpandedMinWidth }
    static var maxExpandedWidth: CGFloat { UIMetrics.sidebarExpandedMaxWidth }
    static var width: CGFloat { UIMetrics.sidebarCollapsedWidth }

    static func clampExpandedWidth(_ value: CGFloat) -> CGFloat {
        min(max(value, minExpandedWidth), maxExpandedWidth)
    }

    static func resolvedWidth(
        expanded: Bool,
        collapsedStyle: SidebarCollapsedStyle,
        expandedStyle: SidebarExpandedStyle,
        expandedCustomWidth: CGFloat? = nil
    ) -> CGFloat {
        if expanded {
            guard expandedStyle == .wide else { return collapsedWidth }
            if let expandedCustomWidth {
                return clampExpandedWidth(expandedCustomWidth)
            }
            return expandedWidth
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
    @Environment(ProjectGroupStore.self) private var projectGroupStore
    @Environment(WorktreeStore.self) private var worktreeStore
    @State private var dragState = ProjectDragState()
    @State private var projectPendingRemoval: Project?
    let expanded: Bool
    let expandedCustomWidth: CGFloat
    @AppStorage(SidebarCollapsedStyle.storageKey) private var collapsedStyleRaw = SidebarCollapsedStyle.defaultValue.rawValue
    @AppStorage(SidebarExpandedStyle.storageKey) private var expandedStyleRaw = SidebarExpandedStyle.defaultValue.rawValue
    @AppStorage(HomeProjectPreferences.visibleKey) private var showHomeProject = HomeProjectPreferences.defaultVisible

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
        .frame(width: SidebarLayout.resolvedWidth(
            expanded: expanded,
            collapsedStyle: collapsedStyle,
            expandedStyle: expandedStyle,
            expandedCustomWidth: expandedCustomWidth
        ))
        .opacity(isHidden ? 0 : 1)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Sidebar")
        .alert(
            "Remove \"\(projectPendingRemoval?.name ?? "")\"?",
            isPresented: removalAlertBinding,
            presenting: projectPendingRemoval
        ) { project in
            Button("Remove", role: .destructive) {
                performRemove(project)
                projectPendingRemoval = nil
            }
            .keyboardShortcut(.defaultAction)
            Button("Cancel", role: .cancel) {
                projectPendingRemoval = nil
            }
            .keyboardShortcut(.cancelAction)
        } message: { _ in
            Text("This will remove the project from Muxy. Project files on disk will not be deleted.")
        }
    }

    private var addButton: some View {
        AddProjectButton(expanded: isWide) {
            ProjectOpenService.openProjectViaPicker(
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore,
                projectGroupStore: projectGroupStore
            )
        }
        .help(shortcutTooltip("Add Project", for: .openProject))
    }

    private var homeProject: Project? {
        showHomeProject ? Project.home : nil
    }

    private var displayedProjects: [Project] {
        projectGroupStore.filteredProjects(from: projectStore.storedProjects)
    }

    private var projectList: some View {
        ScrollView(.vertical, showsIndicators: false) {
            LazyVStack(spacing: UIMetrics.spacing3) {
                WorkspaceSwitcher(isWide: isWide)

                if let homeProject {
                    projectRow(for: homeProject, shortcutIndex: 1)
                }

                ForEach(Array(displayedProjects.enumerated()), id: \.element.id) { offset, project in
                    projectRow(for: project, shortcutIndex: shortcutIndex(forRowAt: offset))
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

    @ViewBuilder
    private func projectRow(for project: Project, shortcutIndex: Int?) -> some View {
        if isWide {
            ExpandedProjectRow(
                project: project,
                shortcutIndex: shortcutIndex,
                isAnyDragging: dragState.draggedID != nil,
                onSelect: { select(project) },
                onRemove: { remove(project) },
                onRename: { projectStore.rename(id: project.id, to: $0) },
                onSetLogo: { projectStore.setLogo(id: project.id, to: $0) },
                onSetIcon: { projectStore.setIcon(id: project.id, to: $0) },
                onSetIconColor: { projectStore.setIconColor(id: project.id, to: $0) }
            )
        } else {
            ProjectRow(
                project: project,
                shortcutIndex: shortcutIndex,
                isAnyDragging: dragState.draggedID != nil,
                onSelect: { select(project) },
                onRemove: { remove(project) },
                onRename: { projectStore.rename(id: project.id, to: $0) },
                onSetLogo: { projectStore.setLogo(id: project.id, to: $0) },
                onSetIcon: { projectStore.setIcon(id: project.id, to: $0) },
                onSetIconColor: { projectStore.setIconColor(id: project.id, to: $0) }
            )
        }
    }

    private func shortcutIndex(forRowAt offset: Int) -> Int? {
        let index = homeProject == nil ? offset + 1 : offset + 2
        return index <= 9 ? index : nil
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
        projectPendingRemoval = project
    }

    private var removalAlertBinding: Binding<Bool> {
        Binding(
            get: { projectPendingRemoval != nil },
            set: { newValue in
                if !newValue {
                    projectPendingRemoval = nil
                }
            }
        )
    }

    private func performRemove(_ project: Project) {
        let capturedProject = project
        let knownWorktrees = worktreeStore.list(for: project.id)
        Task {
            do {
                try await WorktreeStore.cleanupOnDisk(for: capturedProject, knownWorktrees: knownWorktrees)
                appState.removeProject(project.id)
                projectStore.remove(id: project.id)
                worktreeStore.removeProject(project.id)
            } catch {
                presentProjectRemovalFailure(project: capturedProject, error: error)
            }
        }
    }

    private func presentProjectRemovalFailure(project: Project, error: Error) {
        let alert = NSAlert()
        alert.messageText = "Could not remove project \"\(project.name)\""
        alert.informativeText = error.localizedDescription
        alert.alertStyle = .warning
        alert.icon = NSApp.applicationIconImage
        alert.addButton(withTitle: "OK")
        alert.runModal()
    }

    private func reorderIfNeeded(at location: CGPoint) {
        guard let draggedID = dragState.draggedID else { return }
        var hoveredTargetID: UUID?

        for (id, frame) in dragState.frames where id != draggedID {
            guard frame.contains(location) else { continue }
            hoveredTargetID = id
            guard dragState.lastReorderTargetID != id else { return }

            guard let sourceIndex = projectStore.storedProjects.firstIndex(where: { $0.id == draggedID }),
                  let destIndex = projectStore.storedProjects.firstIndex(where: { $0.id == id })
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
    @State private var showThemePicker = false
    @State private var showNotifications = false
    @State private var extensionStore = ExtensionStore.shared

    private var notificationStore: NotificationStore { NotificationStore.shared }

    var body: some View {
        VStack(spacing: 0) {
            if isWide {
                expandedFooter
            } else {
                collapsedFooter
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .toggleThemePicker)) { _ in
            showThemePicker.toggle()
        }
        .onReceive(NotificationCenter.default.publisher(for: .toggleNotificationPanel)) { _ in
            showNotifications.toggle()
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

    private func openExtensions() {
        NotificationCenter.default.post(name: .openExtensionsModal, object: nil)
    }

    private var extensionsHelp: String {
        guard extensionStore.hasUpdates else { return "Extensions" }
        let count = extensionStore.updateCount
        return count == 1 ? "Extensions (1 update available)" : "Extensions (\(count) updates available)"
    }

    private var extensionsAccessibilityLabel: String {
        extensionStore.hasUpdates ? "Extensions, updates available" : "Extensions"
    }

    private var collapsedFooter: some View {
        VStack(spacing: UIMetrics.spacing2) {
            IconButton(symbol: notificationBellIcon, accessibilityLabel: "Notifications") { showNotifications.toggle() }
                .help("Notifications")
                .popover(isPresented: $showNotifications) {
                    NotificationPanel(onDismiss: { showNotifications = false })
                }
            IconButton(
                symbol: "puzzlepiece.extension",
                showsBadge: extensionStore.hasUpdates,
                accessibilityLabel: extensionsAccessibilityLabel
            ) { openExtensions() }
                .help(extensionsHelp)
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

            IconButton(symbol: notificationBellIcon, accessibilityLabel: "Notifications") { showNotifications.toggle() }
                .help("Notifications")
                .popover(isPresented: $showNotifications) {
                    NotificationPanel(onDismiss: { showNotifications = false })
                }
            IconButton(
                symbol: "puzzlepiece.extension",
                showsBadge: extensionStore.hasUpdates,
                accessibilityLabel: extensionsAccessibilityLabel
            ) { openExtensions() }
                .help(extensionsHelp)
            IconButton(symbol: "paintpalette", accessibilityLabel: "Theme Picker") { showThemePicker.toggle() }
                .help("Theme Picker (\(KeyBindingStore.shared.combo(for: .toggleThemePicker).displayString))")
                .popover(isPresented: $showThemePicker) { ThemePicker(mode: .sidebar) }
        }
        .padding(.horizontal, UIMetrics.spacing5)
        .padding(.bottom, UIMetrics.spacing4)
    }
}
