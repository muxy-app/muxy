import SwiftUI

struct VCSWindowView: View {
    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore
    @State private var activeState: VCSTabState?

    private var activeProject: Project? {
        guard let pid = appState.activeProjectID else { return nil }
        return projectStore.projects.first { $0.id == pid }
    }

    var body: some View {
        Group {
            if let state = activeState {
                VCSTabView(state: state, focused: true, onFocus: {})
            } else {
                Text("No project selected")
                    .font(.system(size: UIMetrics.fontEmphasis))
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .frame(minWidth: 500, minHeight: 400)
        .preferredColorScheme(MuxyTheme.colorScheme)
        .onAppear {
            synchronizeState()
        }
        .onChange(of: appState.activeProjectID) {
            synchronizeState()
        }
        .onChange(of: appState.activeWorktreeID) {
            synchronizeState()
        }
        .onChange(of: projectStore.projects.map(\.id)) {
            synchronizeState()
        }
        .onChange(of: worktreeStore.worktrees.mapValues { $0.map(\.id) }) {
            synchronizeState()
        }
    }

    private func synchronizeState() {
        guard let project = activeProject,
              let key = appState.activeWorktreeKey(for: project.id)
        else {
            activeState = nil
            return
        }

        let worktreePath = worktreeStore
            .worktree(projectID: project.id, worktreeID: key.worktreeID)?
            .path ?? project.path
        activeState = VCSStateStore.shared.state(for: worktreePath)
    }
}
