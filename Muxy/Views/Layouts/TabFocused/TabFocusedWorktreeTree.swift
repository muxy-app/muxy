import SwiftUI

struct TabFocusedWorktreeTree: View {
    let project: Project
    let worktrees: [Worktree]
    let shortcutNumbers: [UUID: Int]

    @Environment(AppState.self) private var appState
    @Environment(WorktreeStore.self) private var worktreeStore
    @State private var showCreateWorktreeSheet = false

    var body: some View {
        VStack(spacing: UIMetrics.scaled(1)) {
            ForEach(worktrees) { worktree in
                WorktreeLeafRow(
                    project: project,
                    worktree: worktree,
                    depth: 1,
                    shortcutNumbers: shortcutNumbers
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
        switch result {
        case let .created(worktree, runSetup):
            appState.selectWorktree(projectID: project.id, worktree: worktree)
            if runSetup,
               let paneID = appState.focusedArea(for: project.id)?.activeTab?.content.pane?.id
            {
                Task {
                    await WorktreeSetupRunner.run(
                        sourceProjectPath: project.path,
                        paneID: paneID
                    )
                }
            }
        case .cancelled:
            break
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
