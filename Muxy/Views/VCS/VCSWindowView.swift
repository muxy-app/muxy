import SwiftUI

struct VCSWindowView: View {
    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @State private var vcsStates: [UUID: VCSTabState] = [:]

    private var activeProject: Project? {
        guard let pid = appState.activeProjectID else { return nil }
        return projectStore.projects.first { $0.id == pid }
    }

    private var activeVCSState: VCSTabState? {
        guard let project = activeProject else { return nil }
        if let existing = vcsStates[project.id] {
            return existing
        }
        let state = VCSTabState(projectPath: project.path)
        vcsStates[project.id] = state
        return state
    }

    var body: some View {
        Group {
            if let state = activeVCSState {
                VCSTabView(state: state, focused: true, onFocus: {})
            } else {
                Text("No project selected")
                    .font(.system(size: 13))
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .frame(minWidth: 500, minHeight: 400)
        .preferredColorScheme(.dark)
    }
}
