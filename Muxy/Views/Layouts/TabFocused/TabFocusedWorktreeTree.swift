import SwiftUI

struct TabFocusedWorktreeTree: View {
    let project: Project
    let worktrees: [Worktree]
    let shortcutNumbers: [UUID: Int]
    let content: TabFocusedSidebarContent

    @Environment(AppState.self) private var appState
    @State private var expansionStore = TabFocusedSidebarState.shared
    @State private var showCreateWorktreeSheet = false

    var body: some View {
        VStack(spacing: UIMetrics.scaled(1)) {
            ForEach(worktrees) { worktree in
                WorktreeLeafRow(
                    project: project,
                    worktree: worktree,
                    depth: 1,
                    shortcutNumbers: shortcutNumbers,
                    content: content
                )
            }

            TabFocusedNewWorktreeButton {
                showCreateWorktreeSheet = true
            }
        }
        .padding(.horizontal, TabFocusedSidebarMetrics.rowOuterInset)
        .padding(.top, UIMetrics.spacing1)
        .padding(.bottom, UIMetrics.spacing2)
        .sheet(isPresented: $showCreateWorktreeSheet) {
            CreateWorktreeSheet(project: project) { result in
                showCreateWorktreeSheet = false
                handleCreateWorktreeResult(result)
            }
        }
    }

    private func handleCreateWorktreeResult(_ result: CreateWorktreeResult) {
        guard case let .created(worktree, runSetup) = result else { return }
        appState.selectWorktree(projectID: project.id, worktree: worktree)
        expansionStore.set(worktree.id, expanded: true)
        guard runSetup,
              let paneID = appState.focusedArea(for: project.id)?.activeTab?.content.pane?.id
        else { return }
        Task {
            await WorktreeSetupRunner.run(sourceProjectPath: project.path, paneID: paneID)
        }
    }
}

private struct TabFocusedNewWorktreeButton: View {
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: UIMetrics.spacing3) {
                Image(systemName: "plus")
                    .font(.system(size: UIMetrics.fontCaption, weight: .medium))
                    .foregroundStyle(hovered ? MuxyTheme.accent : MuxyTheme.fg)
                    .frame(width: UIMetrics.scaled(8), height: UIMetrics.scaled(8))
                Text("New Worktree")
                    .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                    .foregroundStyle(hovered ? MuxyTheme.accent : MuxyTheme.fg)
                Spacer()
            }
            .padding(.leading, TabFocusedSidebarMetrics.folderIconSize + TabFocusedSidebarMetrics.iconTitleGap)
            .padding(.trailing, TabFocusedSidebarMetrics.rowHorizontalInset)
            .padding(.vertical, UIMetrics.scaled(5))
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .accessibilityLabel("New Worktree")
    }
}
