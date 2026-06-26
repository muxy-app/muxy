import SwiftUI

struct OverviewWorktreesSection: View {
    let project: Project

    @Environment(AppState.self) private var appState
    @Environment(WorktreeStore.self) private var worktreeStore
    @Environment(ProjectGroupStore.self) private var projectGroupStore

    @State private var showCreateSheet = false
    @State private var isRefreshing = false

    private var worktrees: [Worktree] {
        worktreeStore.list(for: project.id)
    }

    private var activeWorktreeID: UUID? {
        appState.activeWorktreeID[project.id]
    }

    var body: some View {
        OverviewSection(
            title: "Worktrees",
            storageKey: OverviewSidebarPreferences.worktreesSectionExpandedKey,
            accessory: {
                if project.worktreesEnabled, !project.isHome {
                    OverviewActionButton(symbol: "plus", label: "New Worktree") {
                        showCreateSheet = true
                    }
                }
            },
            content: { content }
        )
        .sheet(isPresented: $showCreateSheet) {
            CreateWorktreeSheet(project: project) { result in
                showCreateSheet = false
                handleCreateResult(result)
            }
        }
        .task(id: project.id) { await refresh() }
    }

    @ViewBuilder
    private var content: some View {
        if worktrees.isEmpty {
            Text("No worktrees")
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgMuted)
        } else {
            VStack(spacing: UIMetrics.scaled(1)) {
                ForEach(worktrees) { worktree in
                    OverviewRow(
                        title: displayName(worktree),
                        isSelected: worktree.id == activeWorktreeID,
                        onTap: { appState.selectWorktree(projectID: project.id, worktree: worktree) },
                        leading: {
                            Image(systemName: worktree.isPrimary ? "house" : "arrow.triangle.branch")
                                .font(.system(size: UIMetrics.fontXS, weight: .medium))
                                .foregroundStyle(MuxyTheme.fgMuted)
                                .frame(width: UIMetrics.scaled(14))
                        },
                        trailing: {
                            if worktree.isPrimary {
                                Text("primary")
                                    .font(.system(size: UIMetrics.fontMicro, weight: .semibold))
                                    .foregroundStyle(MuxyTheme.fgMuted)
                            }
                        }
                    )
                }
            }
        }
    }

    private func displayName(_ worktree: Worktree) -> String {
        if worktree.isPrimary, worktree.name.isEmpty { return "main" }
        return worktree.name
    }

    private func handleCreateResult(_ result: CreateWorktreeResult) {
        switch result {
        case let .created(worktree, runSetup):
            appState.selectWorktree(projectID: project.id, worktree: worktree)
            guard runSetup,
                  let paneID = appState.focusedArea(for: project.id)?.activeTab?.content.pane?.id
            else { return }
            Task {
                await WorktreeSetupRunner.run(sourceProjectPath: project.path, paneID: paneID)
            }
        case .cancelled:
            break
        }
    }

    private func refresh() async {
        guard project.worktreesEnabled, !project.isHome else { return }
        await WorktreeRefreshHelper.refresh(
            project: project,
            appState: appState,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore,
            isRefreshing: $isRefreshing,
            presentErrors: false
        )
    }
}
