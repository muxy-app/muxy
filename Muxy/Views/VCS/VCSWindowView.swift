import SwiftUI

struct VCSWindowView: View {
    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @State private var vcsStates: [UUID: VCSTabState] = [:]
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
                    .font(.system(size: 13))
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
        .onChange(of: projectStore.projects.map(\.id)) {
            synchronizeState()
        }
    }

    private func synchronizeState() {
        let validProjectIDs = Set(projectStore.projects.map(\.id))
        vcsStates = vcsStates.filter { validProjectIDs.contains($0.key) }

        guard let project = activeProject else {
            activeState = nil
            return
        }

        if let existing = vcsStates[project.id] {
            activeState = existing
            return
        }

        let state = VCSTabState(projectPath: project.path)
        vcsStates[project.id] = state
        activeState = state
    }
}
