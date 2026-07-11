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

    static func isIcon(
        expanded: Bool,
        collapsedStyle: SidebarCollapsedStyle,
        expandedStyle: SidebarExpandedStyle
    ) -> Bool {
        if expanded {
            return expandedStyle == .icons
        }
        return collapsedStyle == .icons
    }
}

struct ProjectFocusedSidebar: View {
    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(ProjectGroupStore.self) private var projectGroupStore
    @Environment(RemoteDeviceStore.self) private var remoteDeviceStore
    @Environment(WorktreeStore.self) private var worktreeStore
    @State private var dragState = ProjectDragState()
    @State private var isExternalDropTargeted = false
    @State private var projectPendingRemoval: Project?
    let expanded: Bool
    let expandedCustomWidth: CGFloat
    @AppStorage(SidebarCollapsedStyle.storageKey) private var collapsedStyleRaw = SidebarCollapsedStyle.defaultValue.rawValue
    @AppStorage(SidebarExpandedStyle.storageKey) private var expandedStyleRaw = SidebarExpandedStyle.defaultValue.rawValue
    @AppStorage(HomeProjectPreferences.visibleKey) private var showHomeProject = HomeProjectPreferences.defaultVisible
    @AppStorage(ProjectSortMode.storageKey) private var sortModeRaw = ProjectSortMode.defaultValue.rawValue

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
        sidebarContent
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

    private var sidebarContent: some View {
        VStack(spacing: 0) {
            projectList
                .frame(minHeight: 0, maxHeight: .infinity, alignment: .top)
                .clipped()

            SidebarFooter(isWide: isWide, sidebarExpanded: expanded)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    @ViewBuilder private var addButton: some View {
        if projectGroupStore.isRemoteWorkspaceActive {
            AddProjectButton(expanded: isWide, action: openLocalProjectPicker)
                .help(shortcutTooltip("Add Project", for: .openProject))
        } else {
            Menu {
                Button {
                    openLocalProjectPicker()
                } label: {
                    Label("Local", systemImage: "folder")
                }
                remoteProjectMenu
            } label: {
                AddProjectButton.Label(expanded: isWide)
            }
            .menuStyle(.button)
            .menuIndicator(.hidden)
            .buttonStyle(.plain)
            .help(shortcutTooltip("Add Project", for: .openProject))
        }
    }

    private var sortMenu: some View {
        Menu {
            Picker("Sort Projects By", selection: $sortModeRaw) {
                ForEach(ProjectSortMode.allCases) { mode in
                    Label(mode.title, systemImage: mode.systemImage).tag(mode.rawValue)
                }
            }
            .pickerStyle(.inline)
        } label: {
            SortMenuButton.Label(mode: sortMode)
        }
        .menuStyle(.button)
        .menuIndicator(.hidden)
        .buttonStyle(.plain)
        .help("Sort Projects: \(sortMode.title)")
    }

    private var remoteProjectMenu: some View {
        Menu {
            let devices = remoteDeviceStore.sshDevices()
            if devices.isEmpty {
                Button("No devices") {}
                    .disabled(true)
            } else {
                ForEach(devices) { device in
                    Button {
                        NotificationCenter.default.post(
                            name: .openRemoteProjectPicker,
                            object: nil,
                            userInfo: [OpenRemoteProjectPickerUserInfoKey.deviceID: device.id]
                        )
                    } label: {
                        Label(device.displayName, systemImage: "desktopcomputer")
                    }
                }
            }
            Divider()
            Button {
                SettingsFocusCoordinator.shared.request(.remoteDevices)
                NotificationCenter.default.post(name: .openSettingsModal, object: nil)
            } label: {
                Label("Manage Remote Devices", systemImage: "server.rack")
            }
        } label: {
            Label("Remote", systemImage: "network")
        }
    }

    private func openLocalProjectPicker() {
        ProjectOpenService.openProjectViaPicker(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )
    }

    private var homeProject: Project? {
        guard showHomeProject else { return nil }
        guard !projectGroupStore.isRemoteWorkspaceActive else {
            return projectGroupStore.activeRemoteHomeProject
        }
        return Project.home
    }

    private var sortMode: ProjectSortMode {
        ProjectSortMode(rawValue: sortModeRaw) ?? .defaultValue
    }

    private var displayedProjects: [Project] {
        projectGroupStore.displayProjects(localProjects: projectStore.storedProjects, sortMode: sortMode)
    }

    private var pinnedBoundaryIndex: Int? {
        guard let lastPinned = displayedProjects.lastIndex(where: \.isPinned),
              lastPinned < displayedProjects.count - 1
        else { return nil }
        return lastPinned
    }

    private var pinnedDividerOffset: CGFloat {
        UIMetrics.spacing3 / 2
    }

    @ViewBuilder private var listHeader: some View {
        if isWide {
            HStack(spacing: UIMetrics.spacing2) {
                WorkspaceSwitcher(isWide: isWide)
                if showSortMenu {
                    sortMenu
                }
            }
        } else {
            WorkspaceSwitcher(isWide: isWide)
        }
    }

    private var showSortMenu: Bool {
        isWide && !projectGroupStore.isRemoteWorkspaceActive && !displayedProjects.isEmpty
    }

    private var projectList: some View {
        VStack(spacing: UIMetrics.spacing3) {
            listHeader
                .padding(.horizontal, isWide ? UIMetrics.spacing3 : UIMetrics.spacing4)
                .padding(.top, UIMetrics.spacing2)

            scrollableProjects
        }
    }

    private var scrollableProjects: some View {
        ScrollView(.vertical, showsIndicators: false) {
            LazyVStack(spacing: UIMetrics.spacing3) {
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
                        .overlay(alignment: .bottom) {
                            PinnedProjectsDivider()
                                .offset(y: pinnedDividerOffset)
                                .opacity(offset == pinnedBoundaryIndex ? 1 : 0)
                                .allowsHitTesting(false)
                        }
                }

                addButton
            }
            .padding(.horizontal, isWide ? UIMetrics.spacing3 : UIMetrics.spacing4)
            .padding(.bottom, UIMetrics.spacing2)
            .onPreferenceChange(UUIDFramePreferenceKey<SidebarFrameTag>.self) { frames in
                guard dragState.draggedID != nil else { return }
                dragState.frames = frames
            }
        }
        .coordinateSpace(name: "sidebar")
        .overlay {
            if isExternalDropTargeted {
                ExternalProjectDropHighlight()
            }
        }
        .animation(.easeInOut(duration: 0.15), value: isExternalDropTargeted)
        .onDrop(of: [.fileURL], isTargeted: $isExternalDropTargeted) { providers in
            ProjectSidebarDropHandler.handle(providers: providers) { path in
                ProjectOpenService.confirmProjectPathResult(
                    path,
                    appState: appState,
                    projectStore: projectStore,
                    worktreeStore: worktreeStore,
                    projectGroupStore: projectGroupStore
                )
            }
        }
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
                onRename: { renameProject(project, to: $0) },
                onSetLogo: { projectStore.setLogo(id: project.id, to: $0) },
                onSetIcon: { projectStore.setIcon(id: project.id, to: $0) },
                onSetIconColor: { projectStore.setIconColor(id: project.id, to: $0) },
                onSetWorktreesEnabled: { setWorktreesEnabled(project, to: $0) },
                onSetPinned: { projectStore.setPinned(id: project.id, to: $0) }
            )
        } else {
            ProjectRow(
                project: project,
                shortcutIndex: shortcutIndex,
                isAnyDragging: dragState.draggedID != nil,
                onSelect: { select(project) },
                onRemove: { remove(project) },
                onRename: { renameProject(project, to: $0) },
                onSetLogo: { projectStore.setLogo(id: project.id, to: $0) },
                onSetIcon: { projectStore.setIcon(id: project.id, to: $0) },
                onSetIconColor: { projectStore.setIconColor(id: project.id, to: $0) },
                onSetWorktreesEnabled: { setWorktreesEnabled(project, to: $0) },
                onSetPinned: { projectStore.setPinned(id: project.id, to: $0) }
            )
        }
    }

    private func renameProject(_ project: Project, to name: String) {
        guard project.remoteWorkspaceID == nil else {
            projectGroupStore.renameRemoteProject(id: project.id, to: name)
            return
        }
        projectStore.rename(id: project.id, to: name)
    }

    private func setWorktreesEnabled(_ project: Project, to enabled: Bool) {
        guard project.remoteWorkspaceID == nil else {
            projectGroupStore.setRemoteProjectWorktreesEnabled(id: project.id, to: enabled)
            return
        }
        projectStore.setWorktreesEnabled(id: project.id, to: enabled)
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
                    beginManualReorder()
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
        Task {
            do {
                try await ProjectRemovalService.remove(
                    capturedProject,
                    appState: appState,
                    projectStore: projectStore,
                    worktreeStore: worktreeStore,
                    projectGroupStore: projectGroupStore
                )
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

    private func beginManualReorder() {
        guard !projectGroupStore.isRemoteWorkspaceActive else { return }
        projectStore.persistOrder(displayedProjects.map(\.id))
        sortModeRaw = ProjectSortMode.manual.rawValue
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

private struct PinnedProjectsDivider: View {
    var body: some View {
        Rectangle()
            .fill(MuxyTheme.border)
            .frame(height: 1)
            .padding(.horizontal, UIMetrics.spacing2)
            .accessibilityHidden(true)
    }
}

private struct ExternalProjectDropHighlight: View {
    var body: some View {
        RoundedRectangle(cornerRadius: UIMetrics.radiusLG)
            .fill(MuxyTheme.accent.opacity(0.15))
            .overlay(
                RoundedRectangle(cornerRadius: UIMetrics.radiusLG)
                    .strokeBorder(MuxyTheme.accent.opacity(0.6), lineWidth: 2)
            )
            .padding(UIMetrics.spacing2)
            .allowsHitTesting(false)
            .accessibilityHidden(true)
    }
}

private struct AddProjectButton: View {
    var expanded: Bool = false
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Label(expanded: expanded)
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Add Project")
    }

    struct Label: View {
        var expanded: Bool = false
        @State private var hovered = false

        var body: some View {
            Group {
                if expanded {
                    expandedLayout
                } else {
                    collapsedLayout
                }
            }
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
}

private enum SortMenuButton {
    struct Label: View {
        let mode: ProjectSortMode
        @State private var hovered = false

        var body: some View {
            Image(systemName: "arrow.up.arrow.down")
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                .foregroundStyle(hovered ? MuxyTheme.accent : MuxyTheme.fgMuted)
                .frame(width: UIMetrics.controlMedium, height: UIMetrics.controlMedium)
                .background(
                    hovered ? MuxyTheme.hover : MuxyTheme.surface,
                    in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
                )
                .onHover { hovered = $0 }
                .accessibilityLabel("Sort Projects")
        }
    }
}
