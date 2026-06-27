import AppKit
import MuxyShared
import SwiftUI

struct OverviewProjectRow: View {
    let project: Project

    @Environment(AppState.self) private var appState
    @Environment(WorktreeStore.self) private var worktreeStore
    @State private var expansionStore = OverviewExpansionStore.shared
    @State private var notificationStore = NotificationStore.shared
    @State private var progressStore = TerminalProgressStore.shared

    @State private var hovered = false

    private var isActive: Bool {
        appState.activeProjectID == project.id
    }

    private var isExpanded: Bool {
        expansionStore.isExpanded(project.id, default: false)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            if isExpanded {
                OverviewTabsList(project: project)
            }
        }
        .onAppear { applyDefaultExpansion() }
    }

    private var header: some View {
        HStack(spacing: UIMetrics.spacing3) {
            icon
            Text(project.name)
                .font(.system(size: UIMetrics.fontEmphasis, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)
                .lineLimit(1)
                .truncationMode(.tail)
            Spacer(minLength: UIMetrics.spacing2)
            if hovered {
                HStack(spacing: 0) {
                    actions
                    chevron
                }
            } else {
                statusIndicator
            }
        }
        .padding(.horizontal, OverviewSidebarLayout.rowHorizontalInset)
        .padding(.vertical, UIMetrics.spacing3)
        .background(headerBackground)
        .contentShape(Rectangle())
        .onHover { hovered = $0 }
        .onTapGesture { toggle() }
    }

    @ViewBuilder
    private var statusIndicator: some View {
        let unread = notificationStore.unreadCount(for: project.id)
        if progressStore.hasActiveProgress(for: project.id) {
            ProgressView()
                .controlSize(.mini)
                .frame(width: OverviewSidebarLayout.controlSlot, height: OverviewSidebarLayout.controlSlot)
        } else if unread > 0 {
            NotificationBadge(count: unread)
                .frame(width: OverviewSidebarLayout.controlSlot, height: OverviewSidebarLayout.controlSlot)
        } else if progressStore.hasCompletionPending(for: project.id) {
            Circle()
                .fill(MuxyTheme.accent)
                .frame(width: UIMetrics.scaled(8), height: UIMetrics.scaled(8))
                .frame(width: OverviewSidebarLayout.controlSlot, height: OverviewSidebarLayout.controlSlot)
        }
    }

    private var hasMultipleWorktrees: Bool {
        worktreeStore.list(for: project.id).count > 1
    }

    private var isGroupedByWorktree: Bool {
        expansionStore.isGroupedByWorktree(project.id)
    }

    @ViewBuilder
    private var actions: some View {
        if !isGroupedByWorktree {
            OverviewTabActions(project: project, worktree: nil)
        }
        if hasMultipleWorktrees {
            OverviewActionButton(
                symbol: "point.3.connected.trianglepath.dotted",
                label: isGroupedByWorktree ? "Ungroup Worktree Tabs" : "Group Tabs by Worktree",
                isActive: isGroupedByWorktree
            ) {
                expansionStore.setGroupedByWorktree(project.id, grouped: !isGroupedByWorktree)
            }
        }
    }

    private var chevron: some View {
        Button(action: toggle) {
            Image(systemName: "chevron.right")
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
                .rotationEffect(.degrees(isExpanded ? 90 : 0))
                .frame(width: OverviewSidebarLayout.controlSlot, height: OverviewSidebarLayout.controlSlot)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(isExpanded ? "Collapse \(project.name)" : "Expand \(project.name)")
    }

    private var headerBackground: AnyShapeStyle {
        if hovered { return AnyShapeStyle(MuxyTheme.hover) }
        return AnyShapeStyle(Color.clear)
    }

    private var displayLetter: String {
        String(project.name.prefix(1)).uppercased()
    }

    private var icon: some View {
        let logo = resolvedLogo
        return ZStack {
            RoundedRectangle(cornerRadius: UIMetrics.radiusMD, style: .continuous)
                .fill(iconBackground(hasLogo: logo != nil))
            if project.isHome {
                Image(systemName: Project.homeIcon)
                    .font(.system(size: UIMetrics.fontBody, weight: .medium))
                    .foregroundStyle(MuxyTheme.accentForeground)
            } else if let logo {
                Image(nsImage: logo)
                    .resizable()
                    .scaledToFill()
                    .frame(width: UIMetrics.iconXL, height: UIMetrics.iconXL)
                    .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusMD, style: .continuous))
            } else if let iconName = project.icon {
                Image(systemName: iconName)
                    .font(.system(size: UIMetrics.fontBody, weight: .medium))
                    .foregroundStyle(letterForeground)
            } else {
                Text(displayLetter)
                    .font(.system(size: UIMetrics.fontFootnote, weight: .bold))
                    .foregroundStyle(letterForeground)
            }
        }
        .frame(width: UIMetrics.iconXL, height: UIMetrics.iconXL)
    }

    private func iconBackground(hasLogo: Bool) -> AnyShapeStyle {
        if project.isHome { return AnyShapeStyle(MuxyTheme.accent) }
        if hasLogo { return AnyShapeStyle(Color.clear) }
        if let tint = ProjectIconColor.color(for: project.iconColor) {
            return AnyShapeStyle(tint)
        }
        return AnyShapeStyle(MuxyTheme.fg.opacity(0.18))
    }

    private var letterForeground: Color {
        ProjectIconColor.foreground(for: project.iconColor) ?? MuxyTheme.fg
    }

    private var resolvedLogo: NSImage? {
        guard let filename = project.logo,
              let path = ProjectLogoStorage.safeLogoPath(for: filename)
        else { return nil }
        return NSImage(contentsOfFile: path)
    }

    private func toggle() {
        withAnimation(.easeInOut(duration: 0.15)) {
            expansionStore.set(project.id, expanded: !isExpanded)
        }
    }

    private func applyDefaultExpansion() {
        let key = OverviewSidebarPreferences.projectExpandedKey(project.id)
        guard UserDefaults.standard.object(forKey: key) == nil, isActive, !isExpanded else { return }
        expansionStore.set(project.id, expanded: true)
    }
}
