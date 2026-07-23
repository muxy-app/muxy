import SwiftUI

struct TerminalArea: View {
    let project: Project
    let worktreeKey: WorktreeKey
    let isActiveProject: Bool
    @Environment(AppState.self) private var appState
    @Environment(TabDragCoordinator.self) private var dragCoordinator
    @Environment(ProjectGroupStore.self) private var projectGroupStore

    private var workspaceContext: WorkspaceContext {
        projectGroupStore.workspaceContext(for: project)
    }

    private var focusedAreaID: UUID? {
        appState.focusedAreaID[worktreeKey]
    }

    private var visibleLayout: VisiblePaneNode? {
        appState.visibleLayout(for: worktreeKey)
    }

    private var maximizedPane: (area: TabArea, tab: TerminalTab)? {
        guard let areaID = appState.maximizedAreaID[worktreeKey] else { return nil }
        return visibleLayout?.allPanes().first { $0.area.id == areaID }
    }

    var body: some View {
        if let visibleLayout {
            workspaceContent(visibleLayout)
                .environment(\.activeWorktreeKey, worktreeKey)
                .environment(\.paneWorkspaceContext, workspaceContext)
                .onPreferenceChange(AreaFramePreferenceKey.self) { frames in
                    guard isActiveProject, dragCoordinator.activeDrag != nil else { return }
                    dragCoordinator.setAreaFrames(frames, forProject: project.id)
                }
        }
    }

    @ViewBuilder
    private func workspaceContent(_ visibleLayout: VisiblePaneNode) -> some View {
        if let maximizedPane {
            TabAreaView(
                area: maximizedPane.area,
                tab: maximizedPane.tab,
                isFocused: true,
                isActiveProject: isActiveProject,
                projectID: project.id,
                onFocus: {
                    selectPane(areaID: maximizedPane.area.id, tabID: maximizedPane.tab.id)
                },
                onForceCloseTab: {
                    appState.forceCloseTab(
                        maximizedPane.tab.id,
                        areaID: maximizedPane.area.id,
                        projectID: project.id
                    )
                },
                onDropAction: handleDrop
            )
            .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .strokeBorder(MuxyTheme.border, lineWidth: 1)
            )
            .shadow(color: Color.black.opacity(0.35), radius: 24, x: 0, y: 8)
            .padding(16)
        } else {
            PaneNode(
                node: visibleLayout,
                focusedAreaID: focusedAreaID,
                isActiveProject: isActiveProject,
                projectID: project.id,
                onSelectPane: selectPane,
                onForceCloseTab: { areaID, tabID in
                    appState.forceCloseTab(tabID, areaID: areaID, projectID: project.id)
                },
                onDropAction: handleDrop
            )
        }
    }

    private func selectPane(areaID: UUID, tabID: UUID) {
        appState.dispatch(.selectTab(projectID: project.id, areaID: areaID, tabID: tabID))
    }

    private func handleDrop(_ result: TabDragCoordinator.DropResult) {
        appState.dispatch(result.action(projectID: project.id))
    }
}
