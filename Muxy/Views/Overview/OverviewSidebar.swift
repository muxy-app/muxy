import SwiftUI

struct OverviewSidebar: View {
    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore

    private var activeProject: Project? {
        guard let id = appState.activeProjectID else { return nil }
        return projectStore.projects.first { $0.id == id }
    }

    private func activeWorktree(for project: Project) -> Worktree? {
        worktreeStore.preferred(for: project.id, matching: appState.activeWorktreeID[project.id])
    }

    var body: some View {
        Group {
            if let project = activeProject {
                content(for: project)
            } else {
                emptyState
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .background(MuxyTheme.bg)
    }

    private func content(for project: Project) -> some View {
        let worktree = activeWorktree(for: project)
        return ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                OverviewProjectSection(project: project, worktree: worktree)
                divider
                OverviewGitSection(project: project, worktree: worktree)
                divider
                OverviewWorktreesSection(project: project)
                divider
                OverviewTabsSection(project: project)
            }
            .padding(.vertical, UIMetrics.spacing3)
        }
        .scrollIndicators(.never)
    }

    private var divider: some View {
        Rectangle()
            .fill(MuxyTheme.border)
            .frame(height: 1)
            .padding(.horizontal, UIMetrics.spacing4)
            .accessibilityHidden(true)
    }

    private var emptyState: some View {
        VStack(spacing: UIMetrics.spacing4) {
            Image(systemName: "sidebar.right")
                .font(.system(size: UIMetrics.iconXL, weight: .regular))
                .foregroundStyle(MuxyTheme.fgMuted)
            Text("No active project")
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgMuted)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}
