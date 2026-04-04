import SwiftUI

struct TerminalArea: View {
    let project: Project
    let isActiveProject: Bool
    @Environment(AppState.self) private var appState
    @Environment(TabDragCoordinator.self) private var dragCoordinator

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
                    appState.dispatch(.createVCSTab(projectID: project.id, areaID: areaID))
                },
                onCloseTab: { areaID, tabID in
                    appState.dispatch(.closeTab(projectID: project.id, areaID: areaID, tabID: tabID))
                },
                onSplit: { areaID, dir in
                    appState.dispatch(.splitArea(
                        projectID: project.id, areaID: areaID, direction: dir, projectPath: project.path
                    ))
                },
                onCloseArea: { areaID in
                    appState.dispatch(.closeArea(projectID: project.id, areaID: areaID))
                },
                onDropAction: { result in
                    handleDrop(result)
                }
            )
            .onPreferenceChange(AreaFramePreferenceKey.self) { frames in
                dragCoordinator.areaFrames = frames
            }
        }
    }

    private func handleDrop(_ result: TabDragCoordinator.DropResult) {
        let pid = project.id
        let drag = result.drag
        let targetAreaID = result.targetAreaID

        switch result.zone {
        case .center:
            appState.dispatch(.moveTabToArea(
                projectID: pid, tabID: drag.tabID,
                sourceAreaID: drag.sourceAreaID, destinationAreaID: targetAreaID
            ))
        case .left:
            appState.dispatch(.moveTabToNewSplit(
                projectID: pid, tabID: drag.tabID,
                sourceAreaID: drag.sourceAreaID, targetAreaID: targetAreaID,
                direction: .horizontal, position: .first
            ))
        case .right:
            appState.dispatch(.moveTabToNewSplit(
                projectID: pid, tabID: drag.tabID,
                sourceAreaID: drag.sourceAreaID, targetAreaID: targetAreaID,
                direction: .horizontal, position: .second
            ))
        case .top:
            appState.dispatch(.moveTabToNewSplit(
                projectID: pid, tabID: drag.tabID,
                sourceAreaID: drag.sourceAreaID, targetAreaID: targetAreaID,
                direction: .vertical, position: .first
            ))
        case .bottom:
            appState.dispatch(.moveTabToNewSplit(
                projectID: pid, tabID: drag.tabID,
                sourceAreaID: drag.sourceAreaID, targetAreaID: targetAreaID,
                direction: .vertical, position: .second
            ))
        }
    }
}
