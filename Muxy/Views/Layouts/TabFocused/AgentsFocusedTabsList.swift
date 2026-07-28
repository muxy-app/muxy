import SwiftUI

struct AgentsFocusedTabsList: View {
    let project: Project
    let worktree: Worktree

    @Environment(AppState.self) private var appState
    @State private var detectedAgentStore = DetectedAgentStore.shared
    @State private var agentStatusStore = AgentStatusStore.shared

    private var worktreeKey: WorktreeKey {
        WorktreeKey(projectID: project.id, worktreeID: worktree.id)
    }

    private var agentTabs: [AgentsFocusedTabSelection.Location] {
        AgentsFocusedTabSelection.resolve(
            root: appState.workspaceRoots[worktreeKey],
            topLevelTabs: appState.topLevelTabs(for: worktreeKey),
            providerID: providerID
        )
    }

    private var topLevelTabs: [(area: TabArea, tab: TerminalTab)] {
        appState.topLevelTabs(for: worktreeKey)
    }

    var body: some View {
        VStack(spacing: 0) {
            ForEach(agentTabs) { location in
                TabFocusedTabRow(
                    project: project,
                    area: location.area,
                    tab: location.tab,
                    relatedTabs: [location.tab],
                    topLevelTabs: topLevelTabs,
                    active: isActive(location),
                    worktree: worktree
                )
            }
        }
    }

    private func providerID(for paneID: UUID) -> String? {
        detectedAgentStore.agent(for: paneID) ?? agentStatusStore.activeProviderID(forPane: paneID)
    }

    private func isActive(_ location: AgentsFocusedTabSelection.Location) -> Bool {
        appState.activeProjectID == project.id
            && appState.activeWorktreeID[project.id] == worktree.id
            && appState.focusedAreaID[worktreeKey] == location.area.id
            && location.area.activeTabID == location.tab.id
    }
}
