import SwiftUI

struct TerminalArea: View {
    let project: Project
    let isActiveProject: Bool
    @Environment(AppState.self) private var appState
    @Environment(TabDragCoordinator.self) private var dragCoordinator
    @Environment(\.openWindow) private var openWindow

    private var rootIsTabArea: Bool {
        guard let root = appState.workspaceRoot(for: project.id) else { return false }
        if case .tabArea = root { return true }
        return false
    }

    var body: some View {
        if let root = appState.workspaceRoot(for: project.id) {
            PaneNode(
                node: root,
                focusedAreaID: appState.focusedAreaID[project.id],
                isActiveProject: isActiveProject,
                showTabStrip: !rootIsTabArea,
                showVCSButton: false,
                projectID: project.id,
                onFocusArea: { areaID in
                    appState.dispatch(.focusArea(projectID: project.id, areaID: areaID))
                },
                onSelectTab: { areaID, tabID in
                    appState.dispatch(.selectTab(projectID: project.id, areaID: areaID, tabID: tabID))
                },
                onCreateTab: { areaID in
                    appState.dispatch(.createTab(projectID: project.id, areaID: areaID))
                },
                onCreateVCSTab: { areaID in
                    VCSDisplayMode.current.route(
                        tab: { appState.dispatch(.createVCSTab(projectID: project.id, areaID: areaID)) },
                        window: { openWindow(id: "vcs") },
                        attached: { NotificationCenter.default.post(name: .toggleAttachedVCS, object: nil) }
                    )
                },
                onCloseTab: { areaID, tabID in
                    appState.closeTab(tabID, areaID: areaID, projectID: project.id)
                },
                onForceCloseTab: { areaID, tabID in
                    appState.forceCloseTab(tabID, areaID: areaID, projectID: project.id)
                },
                onSplit: { areaID, dir in
                    appState.dispatch(.splitArea(.init(
                        projectID: project.id,
                        areaID: areaID,
                        direction: dir,
                        position: .second,
                        projectPath: project.path
                    )))
                },
                onCloseArea: { areaID in
                    appState.dispatch(.closeArea(projectID: project.id, areaID: areaID))
                },
                onDropAction: { result in
                    appState.dispatch(result.action(projectID: project.id))
                }
            )
            .onPreferenceChange(AreaFramePreferenceKey.self) { frames in
                dragCoordinator.setAreaFrames(frames, forProject: project.id)
            }
        }
    }
}
