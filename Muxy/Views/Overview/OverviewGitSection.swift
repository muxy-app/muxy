import SwiftUI

struct OverviewGitSection: View {
    let project: Project
    let worktree: Worktree?

    @Environment(ProjectGroupStore.self) private var projectGroupStore

    @State private var snapshot: GitStatusSnapshot?
    @State private var isLoading = false
    @State private var isGitRepo = true

    private var repoPath: String { worktree?.path ?? project.path }

    var body: some View {
        OverviewSection(
            title: "Git",
            storageKey: OverviewSidebarPreferences.gitSectionExpandedKey,
            accessory: {
                if isLoading {
                    ProgressView().controlSize(.mini)
                }
            },
            content: { content }
        )
        .task(id: taskID) { await load() }
    }

    private var taskID: String {
        "\(repoPath)|\(project.isRemote)"
    }

    @ViewBuilder
    private var content: some View {
        if project.isHome {
            emptyState("Not a repository")
        } else if !isGitRepo {
            emptyState("Not a repository")
        } else if let snapshot {
            VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
                branchRow(snapshot)
                statusRow(snapshot)
            }
        } else if !isLoading {
            emptyState("No git info")
        }
    }

    private func branchRow(_ snapshot: GitStatusSnapshot) -> some View {
        HStack(spacing: UIMetrics.spacing2) {
            Image(systemName: "arrow.triangle.branch")
                .font(.system(size: UIMetrics.fontXS, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
            Text(snapshot.branch)
                .font(.system(size: UIMetrics.fontFootnote, weight: .medium, design: .monospaced))
                .foregroundStyle(MuxyTheme.fg)
                .lineLimit(1)
                .truncationMode(.middle)
            Spacer(minLength: 0)
            aheadBehind(snapshot.aheadBehind)
        }
    }

    @ViewBuilder
    private func aheadBehind(_ value: GitRepositoryService.AheadBehind) -> some View {
        if value.hasUpstream, value.ahead > 0 || value.behind > 0 {
            HStack(spacing: UIMetrics.spacing2) {
                if value.ahead > 0 {
                    counter(symbol: "arrow.up", count: value.ahead)
                }
                if value.behind > 0 {
                    counter(symbol: "arrow.down", count: value.behind)
                }
            }
        }
    }

    private func counter(symbol: String, count: Int) -> some View {
        HStack(spacing: UIMetrics.scaled(1)) {
            Image(systemName: symbol)
                .font(.system(size: UIMetrics.fontMicro, weight: .bold))
            Text("\(count)")
                .font(.system(size: UIMetrics.fontXS, weight: .semibold))
        }
        .foregroundStyle(MuxyTheme.fgMuted)
    }

    private func statusRow(_ snapshot: GitStatusSnapshot) -> some View {
        let changed = snapshot.files.count
        return HStack(spacing: UIMetrics.spacing2) {
            Circle()
                .fill(changed > 0 ? MuxyTheme.warning : MuxyTheme.diffAddFg)
                .frame(width: UIMetrics.scaled(6), height: UIMetrics.scaled(6))
            Text(changed > 0 ? "\(changed) changed" : "Clean")
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgMuted)
            Spacer(minLength: 0)
        }
    }

    private func emptyState(_ text: String) -> some View {
        Text(text)
            .font(.system(size: UIMetrics.fontFootnote))
            .foregroundStyle(MuxyTheme.fgMuted)
    }

    private func load() async {
        guard !project.isHome else {
            isGitRepo = false
            return
        }
        isLoading = true
        defer { isLoading = false }
        let context = projectGroupStore.workspaceContext(for: project)
        let service = GitRepositoryService(context: context)
        do {
            snapshot = try await GitStatusAggregator.snapshot(
                repoPath: repoPath,
                includePullRequest: false,
                git: service
            )
            isGitRepo = true
        } catch {
            snapshot = nil
            isGitRepo = false
        }
    }
}
