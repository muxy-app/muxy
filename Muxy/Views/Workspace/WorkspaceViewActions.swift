import Foundation

@MainActor
struct WorkspaceViewActions {
    let projectID: UUID
    let focusArea: (UUID) -> Void
    let selectTab: (UUID, UUID) -> Void
    let createTab: (UUID) -> Void
    let closeTab: (UUID, UUID) -> Void
    let forceCloseTab: (UUID, UUID) -> Void
    let splitArea: (UUID, SplitDirection, SplitPosition) -> Void
    let closeArea: (UUID) -> Void
    var dropTab: ((TabDragCoordinator.DropResult) -> Void)?
    var createBrowserTab: ((UUID) -> Void)?
    var createTabAdjacent: ((UUID, UUID, TabArea.InsertSide) -> Void)?
    var togglePin: ((UUID, UUID) -> Void)?
    var setCustomTitle: ((UUID, UUID, String?) -> Void)?
    var setColorID: ((UUID, UUID, String?) -> Void)?
    var reorderTab: ((UUID, IndexSet, Int) -> Void)?
    var resizeSplit: ((UUID, CGFloat) -> Void)?

    static func local(projectID: UUID, appState: AppState) -> WorkspaceViewActions {
        WorkspaceViewActions(
            projectID: projectID,
            focusArea: { areaID in
                appState.dispatch(.focusArea(projectID: projectID, areaID: areaID))
            },
            selectTab: { areaID, tabID in
                appState.dispatch(.selectTab(projectID: projectID, areaID: areaID, tabID: tabID))
            },
            createTab: { areaID in
                appState.dispatch(.createTab(projectID: projectID, areaID: areaID))
            },
            closeTab: { areaID, tabID in
                appState.closeTab(tabID, areaID: areaID, projectID: projectID)
            },
            forceCloseTab: { areaID, tabID in
                appState.forceCloseTab(tabID, areaID: areaID, projectID: projectID)
            },
            splitArea: { areaID, direction, position in
                appState.dispatch(.splitArea(.init(
                    projectID: projectID,
                    areaID: areaID,
                    direction: direction,
                    position: position
                )))
            },
            closeArea: { areaID in
                appState.dispatch(.closeArea(projectID: projectID, areaID: areaID))
            },
            dropTab: { result in
                appState.dispatch(result.action(projectID: projectID))
            },
            createBrowserTab: { areaID in
                appState.dispatch(.createBrowserTab(
                    projectID: projectID,
                    areaID: areaID,
                    url: BrowserURL.homeURL,
                    profileID: BrowserPreferences.defaultProfileID
                ))
            },
            createTabAdjacent: { areaID, tabID, side in
                appState.dispatch(.createTabAdjacent(
                    projectID: projectID,
                    areaID: areaID,
                    tabID: tabID,
                    side: side
                ))
            },
            togglePin: { areaID, tabID in
                appState.workspaceRoot(for: projectID)?.findArea(id: areaID)?.togglePin(tabID)
            },
            setCustomTitle: { areaID, tabID, title in
                appState.workspaceRoot(for: projectID)?.findArea(id: areaID)?.setCustomTitle(tabID, title: title)
                appState.saveWorkspaces()
            },
            setColorID: { areaID, tabID, colorID in
                appState.workspaceRoot(for: projectID)?.findArea(id: areaID)?.setColorID(tabID, colorID: colorID)
                appState.saveWorkspaces()
            },
            reorderTab: { areaID, source, destination in
                appState.workspaceRoot(for: projectID)?.findArea(id: areaID)?.reorderTab(
                    fromOffsets: source,
                    toOffset: destination
                )
            },
            resizeSplit: { branchID, ratio in
                appState.workspaceRoot(for: projectID)?.findBranch(id: branchID)?.ratio = ratio
            }
        )
    }
}
