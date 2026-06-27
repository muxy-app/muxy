import AppKit
import MuxyShared
import SwiftUI

struct TabFocusedProjectPopover: View {
    let onDismiss: () -> Void

    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore
    @Environment(ProjectGroupStore.self) private var projectGroupStore
    @AppStorage(HomeProjectPreferences.visibleKey) private var showHomeProject = HomeProjectPreferences.defaultVisible

    private var projects: [Project] {
        let stored = projectGroupStore.displayProjects(localProjects: projectStore.storedProjects)
        guard showHomeProject else { return stored }
        if projectGroupStore.isRemoteWorkspaceActive {
            guard let home = projectGroupStore.activeRemoteHomeProject else { return stored }
            return [home] + stored
        }
        return [Project.home] + stored
    }

    private var activeID: UUID? {
        appState.activeProjectID
    }

    var body: some View {
        PopoverPicker(
            items: projects,
            filterKey: { $0.name },
            searchPlaceholder: "Search projects…",
            emptyLabel: "No projects",
            onSelect: { select($0) },
            row: { project, isHighlighted in
                row(project, isHighlighted: isHighlighted)
            }
        )
    }

    private func row(_ project: Project, isHighlighted: Bool) -> some View {
        let selected = project.id == activeID
        return HStack(spacing: UIMetrics.spacing3) {
            icon(for: project)
            Text(project.name)
                .font(.system(size: UIMetrics.fontBody, weight: selected ? .semibold : .regular))
                .foregroundStyle(MuxyTheme.fg)
                .lineLimit(1)
                .truncationMode(.middle)
            Spacer(minLength: UIMetrics.spacing1)
            if selected {
                Image(systemName: "checkmark")
                    .font(.system(size: UIMetrics.fontXS, weight: .semibold))
                    .foregroundStyle(MuxyTheme.accent)
            }
        }
        .padding(.horizontal, UIMetrics.spacing4)
        .padding(.vertical, UIMetrics.scaled(7))
        .background(rowBackground(selected: selected, isHighlighted: isHighlighted), in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
        .padding(.horizontal, UIMetrics.spacing3)
        .padding(.vertical, UIMetrics.spacing1)
        .contentShape(Rectangle())
    }

    private func icon(for project: Project) -> some View {
        ZStack {
            RoundedRectangle(cornerRadius: UIMetrics.radiusSM, style: .continuous)
                .fill(iconBackground(for: project))
            if project.isHome {
                Image(systemName: Project.homeIcon)
                    .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                    .foregroundStyle(MuxyTheme.accentForeground)
            } else if let iconName = project.icon {
                Image(systemName: iconName)
                    .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                    .foregroundStyle(letterForeground(for: project))
            } else {
                Text(String(project.name.prefix(1)).uppercased())
                    .font(.system(size: UIMetrics.fontXS, weight: .bold))
                    .foregroundStyle(letterForeground(for: project))
            }
        }
        .frame(width: UIMetrics.iconLG, height: UIMetrics.iconLG)
    }

    private func iconBackground(for project: Project) -> AnyShapeStyle {
        if project.isHome { return AnyShapeStyle(MuxyTheme.accent) }
        if let tint = ProjectIconColor.color(for: project.iconColor) {
            return AnyShapeStyle(tint)
        }
        return AnyShapeStyle(MuxyTheme.fg.opacity(0.18))
    }

    private func letterForeground(for project: Project) -> Color {
        ProjectIconColor.foreground(for: project.iconColor) ?? MuxyTheme.fg
    }

    private func rowBackground(selected: Bool, isHighlighted: Bool) -> AnyShapeStyle {
        if selected { return AnyShapeStyle(MuxyTheme.accentSoft) }
        if isHighlighted { return AnyShapeStyle(MuxyTheme.surface) }
        return AnyShapeStyle(Color.clear)
    }

    private func select(_ project: Project) {
        worktreeStore.ensurePrimary(for: project)
        if let worktree = worktreeStore.preferred(for: project.id, matching: appState.activeWorktreeID[project.id]) {
            appState.selectProject(project, worktree: worktree)
        }
        onDismiss()
    }
}
