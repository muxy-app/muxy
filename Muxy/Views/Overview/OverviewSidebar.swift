import SwiftUI

struct OverviewSidebar: View {
    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore
    @Environment(ProjectGroupStore.self) private var projectGroupStore
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
        guard let homeProject else { return stored }
        return [homeProject] + stored
    }

    var body: some View {
        VStack(spacing: 0) {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(projects) { project in
                        OverviewProjectRow(project: project)
                    }
                    OverviewAddProjectRow(action: openProjectPicker)
                }
                .padding(.vertical, UIMetrics.spacing3)
            }
            .scrollIndicators(.never)

            Rectangle().fill(MuxyTheme.border).frame(height: 1)
                .accessibilityHidden(true)

            OverviewFooter()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .background(MuxyTheme.bg)
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

private struct OverviewAddProjectRow: View {
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
            .padding(.horizontal, OverviewSidebarLayout.rowHorizontalInset)
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

private struct OverviewFooter: View {
    @State private var extensionStore = ExtensionStore.shared
    @State private var notificationStore = NotificationStore.shared
    @State private var showThemePicker = false
    @State private var showNotifications = false

    private var notificationBellIcon: String {
        notificationStore.unreadCount > 0 ? "bell.badge" : "bell"
    }

    private var extensionsHelp: String {
        guard extensionStore.hasUpdates else { return "Extensions" }
        let count = extensionStore.updateCount
        return count == 1 ? "Extensions (1 update available)" : "Extensions (\(count) updates available)"
    }

    var body: some View {
        HStack(spacing: UIMetrics.spacing2) {
            IconButton(symbol: "sidebar.left", accessibilityLabel: "Hide Project Overview") {
                NotificationCenter.default.post(name: .toggleOverviewSidebar, object: nil)
            }
            .help("Hide Project Overview")

            Spacer()

            IconButton(symbol: notificationBellIcon, accessibilityLabel: "Notifications") { showNotifications.toggle() }
                .help("Notifications")
                .popover(isPresented: $showNotifications) {
                    NotificationPanel(onDismiss: { showNotifications = false })
                }
            IconButton(
                symbol: "puzzlepiece.extension",
                showsBadge: extensionStore.hasUpdates,
                accessibilityLabel: extensionStore.hasUpdates ? "Extensions, updates available" : "Extensions"
            ) { NotificationCenter.default.post(name: .openExtensionsModal, object: nil) }
                .help(extensionsHelp)
            IconButton(symbol: "paintpalette", accessibilityLabel: "Theme Picker") { showThemePicker.toggle() }
                .help("Theme Picker (\(KeyBindingStore.shared.combo(for: .toggleThemePicker).displayString))")
                .popover(isPresented: $showThemePicker) { ThemePicker(mode: .sidebar) }
        }
        .padding(.horizontal, UIMetrics.spacing5)
        .padding(.vertical, UIMetrics.spacing3)
        .onReceive(NotificationCenter.default.publisher(for: .toggleThemePicker)) { _ in
            showThemePicker.toggle()
        }
        .onReceive(NotificationCenter.default.publisher(for: .toggleNotificationPanel)) { _ in
            showNotifications.toggle()
        }
    }
}
