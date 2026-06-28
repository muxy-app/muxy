import SwiftUI

struct TabFocusedSidebar: View {
    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore
    @Environment(ProjectGroupStore.self) private var projectGroupStore
    @State private var expansionStore = TabFocusedSidebarState.shared
    @AppStorage(HomeProjectPreferences.visibleKey) private var showHomeProject = HomeProjectPreferences.defaultVisible
    @AppStorage(ProjectSortMode.storageKey) private var sortModeRaw = ProjectSortMode.defaultValue.rawValue

    private var sortMode: ProjectSortMode {
        ProjectSortMode(rawValue: sortModeRaw) ?? .defaultValue
    }

    private var homeProject: Project? {
        guard showHomeProject else { return nil }
        guard !projectGroupStore.isRemoteWorkspaceActive else {
            return projectGroupStore.activeRemoteHomeProject
        }
        return Project.home
    }

    private var projects: [Project] {
        let stored = projectGroupStore.displayProjects(localProjects: projectStore.storedProjects, sortMode: sortMode)
        let all = homeProject.map { [$0] + stored } ?? stored
        guard expansionStore.focusMode,
              let activeID = appState.activeProjectID,
              let focused = all.first(where: { $0.id == activeID })
        else { return all }
        return [focused]
    }

    private var shortcutNumbers: [UUID: Int] {
        let entries = TabFocusedTabOrder.entries(
            appState: appState,
            projectStore: projectStore,
            projectGroupStore: projectGroupStore,
            worktreeStore: worktreeStore
        )
        var map: [UUID: Int] = [:]
        for (index, entry) in entries.prefix(9).enumerated() {
            map[entry.tabID] = index + 1
        }
        return map
    }

    var body: some View {
        let numbers = shortcutNumbers
        return VStack(spacing: 0) {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(Array(projects.enumerated()), id: \.element.id) { offset, project in
                        TabFocusedProjectRow(
                            project: project,
                            shortcutNumbers: numbers,
                            projectShortcutIndex: projectShortcutIndex(forRowAt: offset)
                        )
                    }
                    if !expansionStore.focusMode {
                        TabFocusedAddProjectRow(action: openProjectPicker)
                    }
                }
                .padding(.bottom, UIMetrics.spacing3)
            }
            .scrollIndicators(.never)

            SidebarFooter()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .background(MuxyTheme.bg)
    }

    private func projectShortcutIndex(forRowAt offset: Int) -> Int? {
        let index = offset + 1
        return index <= 9 ? index : nil
    }

    private func openProjectPicker() {
        ProjectOpenService.openProjectViaPicker(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )
    }
}

private struct TabFocusedAddProjectRow: View {
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: UIMetrics.spacing3) {
                ZStack {
                    RoundedRectangle(cornerRadius: UIMetrics.radiusMD, style: .continuous)
                        .fill(MuxyTheme.surface)
                    Image(systemName: "plus")
                        .font(.system(size: UIMetrics.fontFootnote, weight: .bold))
                        .foregroundStyle(hovered ? MuxyTheme.accent : MuxyTheme.fgMuted)
                }
                .frame(width: UIMetrics.iconXL, height: UIMetrics.iconXL)
                Text("Add Project")
                    .font(.system(size: UIMetrics.fontEmphasis, weight: .medium))
                    .foregroundStyle(hovered ? MuxyTheme.accent : MuxyTheme.fgMuted)
                Spacer(minLength: 0)
            }
            .padding(.horizontal, TabFocusedSidebarMetrics.rowHorizontalInset)
            .padding(.vertical, UIMetrics.spacing3)
            .background(hovered ? MuxyTheme.hover : Color.clear)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(shortcutTooltip)
        .accessibilityLabel("Add Project")
    }

    private var shortcutTooltip: String {
        "Add Project (\(KeyBindingStore.shared.combo(for: .openProject).displayString))"
    }
}
