import SwiftUI

struct SidebarToolbar: View {
    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @State private var showThemePicker = false

    var body: some View {
        HStack(spacing: 4) {
            Spacer()
            IconButton(symbol: "paintpalette") { showThemePicker.toggle() }
                .popover(isPresented: $showThemePicker) { ThemePicker() }
            IconButton(symbol: "plus") { addProject() }
            IconButton(symbol: "sidebar.left") {
                withAnimation(.easeInOut(duration: 0.2)) {
                    appState.sidebarVisible.toggle()
                }
            }
        }
        .padding(.horizontal, 10)
        .frame(height: 32)
        .background(WindowDragRepresentable())
    }

    private func addProject() {
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
}

struct Sidebar: View {
    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore

    var body: some View {
        ScrollView(.vertical, showsIndicators: false) {
            VStack(spacing: 2) {
                ForEach(Array(projectStore.projects.enumerated()), id: \.element.id) { index, project in
                    ProjectItem(
                        project: project,
                        selected: project.id == appState.activeProjectID,
                        shortcutIndex: index < 9 ? index + 1 : nil,
                        onSelect: { appState.selectProject(project) },
                        onRemove: {
                            appState.removeProject(project.id)
                            projectStore.remove(id: project.id)
                        }
                    )
                }
            }
            .padding(6)
        }
    }
}

private struct ProjectItem: View {
    let project: Project
    let selected: Bool
    var shortcutIndex: Int?
    let onSelect: () -> Void
    let onRemove: () -> Void
    @State private var hovered = false

    private var showBadge: Bool {
        guard shortcutIndex != nil else { return false }
        return ModifierKeyMonitor.shared.controlHeld
    }

    var body: some View {
        Text(project.name)
            .font(.system(size: 12))
            .foregroundStyle(selected ? MuxyTheme.accent : MuxyTheme.fgMuted)
            .lineLimit(1)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(background, in: RoundedRectangle(cornerRadius: 6))
            .contentShape(RoundedRectangle(cornerRadius: 6))
            .overlay(alignment: .trailing) {
                if showBadge, let shortcutIndex {
                    ShortcutBadge(label: "⌃\(shortcutIndex)")
                        .padding(.trailing, 6)
                }
            }
            .onTapGesture(perform: onSelect)
            .onHover { hovered = $0 }
            .contextMenu {
                Button("Remove Project", role: .destructive, action: onRemove)
            }
    }

    private var background: some ShapeStyle {
        if selected { return AnyShapeStyle(MuxyTheme.accentSoft) }
        if hovered { return AnyShapeStyle(MuxyTheme.hover) }
        return AnyShapeStyle(.clear)
    }
}
