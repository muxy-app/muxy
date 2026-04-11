import SwiftUI

struct SidebarToolbar: View {
    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore

    var body: some View {
        HStack(spacing: 4) {
            Spacer()
            IconButton(symbol: "folder") {
                ProjectOpenService.openProject(
                    appState: appState,
                    projectStore: projectStore,
                    worktreeStore: worktreeStore
                )
            }
            .help(shortcutTooltip("Open Project", for: .openProject))
            IconButton(symbol: "sidebar.left") {
                withAnimation(.easeInOut(duration: 0.2)) {
                    appState.sidebarVisible.toggle()
                }
            }
            .help(shortcutTooltip("Toggle Sidebar", for: .toggleSidebar))
        }
        .padding(.horizontal, 10)
        .frame(height: 32)
        .background(WindowDragRepresentable())
    }

    private func shortcutTooltip(_ name: String, for action: ShortcutAction) -> String {
        "\(name) (\(KeyBindingStore.shared.combo(for: action).displayString))"
    }
}

struct Sidebar: View {
    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore
    @State private var dragState = ProjectDragState()

    var body: some View {
        VStack(spacing: 0) {
            if projectStore.projects.isEmpty {
                emptyState
            } else {
                projectList
            }
            Spacer(minLength: 0)
            SidebarFooter()
        }
    }

    private var projectList: some View {
        ScrollView(.vertical, showsIndicators: false) {
            LazyVStack(spacing: 2) {
                ForEach(Array(projectStore.projects.enumerated()), id: \.element.id) { index, project in
                    ProjectRow(
                        project: project,
                        shortcutIndex: index < 9 ? index + 1 : nil,
                        isAnyDragging: dragState.draggedID != nil,
                        onSelect: { select(project) },
                        onRemove: { remove(project) },
                        onRename: { projectStore.rename(id: project.id, to: $0) }
                    )
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
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 10)
            .onPreferenceChange(UUIDFramePreferenceKey<SidebarFrameTag>.self) { frames in
                guard dragState.draggedID != nil else { return }
                dragState.frames = frames
            }
        }
        .coordinateSpace(name: "sidebar")
    }

    private var emptyState: some View {
        VStack(spacing: 10) {
            Image(systemName: "folder.badge.plus")
                .font(.system(size: 26, weight: .regular))
                .foregroundStyle(MuxyTheme.fgDim)
            Text("No projects yet")
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(MuxyTheme.fgMuted)
            Text("Click \(Image(systemName: "folder")) above to open one")
                .font(.system(size: 10))
                .foregroundStyle(MuxyTheme.fgDim)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity)
        .padding(.horizontal, 16)
        .padding(.vertical, 32)
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
        let list = worktreeStore.list(for: project.id)
        let existingID = appState.activeWorktreeID[project.id]
        let worktree = list.first(where: { $0.id == existingID })
            ?? list.first(where: { $0.isPrimary })
            ?? list.first
        guard let worktree else { return }
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

struct SidebarFooter: View {
    @State private var showThemePicker = false

    private var versionString: String {
        Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "dev"
    }

    var body: some View {
        VStack(spacing: 0) {
            Rectangle().fill(MuxyTheme.border).frame(height: 1)
            HStack(spacing: 4) {
                Text("Muxy \(versionString)")
                    .font(.system(size: 11))
                    .foregroundStyle(MuxyTheme.fgMuted)
                Spacer()
                IconButton(symbol: "paintpalette") { showThemePicker.toggle() }
                    .help("Theme Picker (\(KeyBindingStore.shared.combo(for: .toggleThemePicker).displayString))")
                    .popover(isPresented: $showThemePicker) { ThemePicker() }
            }
            .padding(.horizontal, 10)
            .frame(height: 32)
        }
        .onReceive(NotificationCenter.default.publisher(for: .toggleThemePicker)) { _ in
            showThemePicker.toggle()
        }
    }
}
