import AppKit
import SwiftUI

struct SidebarToolbar: View {
    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore

    private var showHints: Bool { ModifierKeyMonitor.shared.showHints }

    var body: some View {
        HStack(spacing: 4) {
            Spacer()
            IconButton(symbol: "folder") { openProject() }
            IconButton(symbol: "plus") { addProject() }
            IconButton(symbol: "sidebar.left") {
                withAnimation(.easeInOut(duration: 0.2)) {
                    appState.sidebarVisible.toggle()
                }
            }
        }
        .padding(.horizontal, 10)
        .frame(height: 32)
        .overlay(alignment: .trailing) {
            if showHints {
                HStack(spacing: 3) {
                    ShortcutBadge(
                        label: KeyBindingStore.shared.combo(for: .openProject).displayString,
                        compact: true
                    )
                    ShortcutBadge(
                        label: KeyBindingStore.shared.combo(for: .newProject).displayString,
                        compact: true
                    )
                    ShortcutBadge(
                        label: KeyBindingStore.shared.combo(for: .toggleSidebar).displayString,
                        compact: true
                    )
                }
                .padding(.horizontal, 10)
                .allowsHitTesting(false)
            }
        }
        .background(WindowDragRepresentable())
    }

    private func openProject() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.message = "Select a project folder"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        let project = Project(
            name: url.lastPathComponent,
            path: url.path(percentEncoded: false),
            sortOrder: projectStore.projects.count
        )
        projectStore.add(project)
        appState.selectProject(project)
    }

    private func addProject() {
        let url = FileManager.default.homeDirectoryForCurrentUser
        let project = Project(
            name: url.lastPathComponent,
            path: url.path(percentEncoded: false),
            sortOrder: projectStore.projects.count
        )
        projectStore.add(project)
        appState.selectProject(project)
    }
}

struct Sidebar: View {
    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @State private var dragState = ProjectDragState()

    var body: some View {
        VStack(spacing: 0) {
            ScrollView(.vertical, showsIndicators: false) {
                VStack(spacing: 2) {
                    ForEach(Array(projectStore.projects.enumerated()), id: \.element.id) {
                        index, project in
                        ProjectItem(
                            project: project,
                            selected: project.id == appState.activeProjectID,
                            isAnyDragging: dragState.draggedID != nil,
                            shortcutIndex: index < 9 ? index + 1 : nil,
                            onRemove: {
                                appState.removeProject(project.id)
                                projectStore.remove(id: project.id)
                            },
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
                        .gesture(
                            DragGesture(minimumDistance: 4, coordinateSpace: .named("sidebar"))
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
                        )
                        .onTapGesture {
                            guard dragState.draggedID == nil else { return }
                            appState.selectProject(project)
                        }
                    }
                }
                .padding(6)
                .onPreferenceChange(UUIDFramePreferenceKey<SidebarFrameTag>.self) { frames in
                    guard dragState.draggedID != nil else { return }
                    dragState.frames = frames
                }
            }
            .coordinateSpace(name: "sidebar")
            SidebarFooter()
        }
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

private struct ProjectItem: View {
    let project: Project
    let selected: Bool
    var isAnyDragging: Bool = false
    var shortcutIndex: Int?
    let onRemove: () -> Void
    let onRename: (String) -> Void
    @State private var hovered = false
    @State private var isRenaming = false
    @State private var renameText = ""
    @FocusState private var renameFieldFocused: Bool

    private var showBadge: Bool {
        guard let shortcutIndex,
              let action = ShortcutAction.projectAction(for: shortcutIndex)
        else { return false }
        return ModifierKeyMonitor.shared.isHolding(
            modifiers: KeyBindingStore.shared.combo(for: action).modifiers
        )
    }

    var body: some View {
        Group {
            if isRenaming {
                TextField("", text: $renameText)
                    .textFieldStyle(.plain)
                    .font(.system(size: 12))
                    .foregroundStyle(MuxyTheme.fg)
                    .focused($renameFieldFocused)
                    .onSubmit { commitRename() }
                    .onExitCommand { cancelRename() }
            } else {
                Text(project.name)
                    .font(.system(size: 12))
                    .foregroundStyle(selected ? MuxyTheme.accent : MuxyTheme.fgMuted)
            }
        }
        .lineLimit(1)
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(background, in: RoundedRectangle(cornerRadius: 6))
        .contentShape(RoundedRectangle(cornerRadius: 6))
        .overlay(alignment: .trailing) {
            if showBadge, let shortcutIndex,
               let action = ShortcutAction.projectAction(for: shortcutIndex)
            {
                ShortcutBadge(label: KeyBindingStore.shared.combo(for: action).displayString)
                    .padding(.trailing, 6)
            }
        }
        .onHover { hovering in
            guard !isAnyDragging else { return }
            hovered = hovering
        }
        .onChange(of: isAnyDragging) { _, dragging in
            if dragging { hovered = false }
        }
        .contextMenu {
            Button("Rename Project") { startRename() }
            Divider()
            Button("Remove Project", role: .destructive, action: onRemove)
        }
    }

    private var background: some ShapeStyle {
        if selected { return AnyShapeStyle(MuxyTheme.accentSoft) }
        if hovered { return AnyShapeStyle(MuxyTheme.hover) }
        return AnyShapeStyle(.clear)
    }

    private func startRename() {
        renameText = project.name
        isRenaming = true
        renameFieldFocused = true
    }

    private func commitRename() {
        let trimmed = renameText.trimmingCharacters(in: .whitespaces)
        if !trimmed.isEmpty {
            onRename(trimmed)
        }
        isRenaming = false
    }

    private func cancelRename() {
        isRenaming = false
    }
}
