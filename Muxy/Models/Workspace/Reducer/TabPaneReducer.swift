import Foundation

@MainActor
enum TabPaneReducer {
    @discardableResult
    static func splitTabPane(
        projectID: UUID,
        areaID: UUID,
        tabID: UUID,
        direction: SplitDirection,
        state: inout WorkspaceState
    ) -> UUID? {
        guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state),
              let root = state.workspaceRoots[key],
              let area = root.findArea(id: areaID),
              let tab = area.tabs.first(where: { $0.id == tabID }),
              case .terminal = tab.content,
              let pane = tab.content.pane
        else { return nil }

        if let internalPanes = tab.internalPanes {
            let (newNode, newPaneID) = internalPanes.splitting(
                paneID: tab.focusedPaneID ?? pane.id,
                direction: direction,
                position: .second
            )
            tab.internalPanes = newNode
            if let newPaneID {
                tab.focusedPaneID = newPaneID
            }
            return newPaneID
        }

        let newPane = TerminalPaneState(projectPath: area.projectPath)
        let branch = InternalBranch(
            direction: direction,
            first: .pane(pane),
            second: .pane(newPane)
        )
        tab.internalPanes = .split(branch)
        tab.focusedPaneID = newPane.id
        return newPane.id
    }

    @discardableResult
    static func closeInternalPane(
        projectID: UUID,
        areaID: UUID,
        tabID: UUID,
        paneID: UUID,
        state: inout WorkspaceState
    ) -> UUID? {
        guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state),
              let root = state.workspaceRoots[key],
              let area = root.findArea(id: areaID),
              let tab = area.tabs.first(where: { $0.id == tabID }),
              let internalPanes = tab.internalPanes
        else { return nil }

        let allPanes = internalPanes.allPanes()
        guard allPanes.count > 1 else {
            tab.internalPanes = nil
            tab.focusedPaneID = nil
            return paneID
        }

        if let remaining = internalPanes.removing(paneID: paneID) {
            let remainingPanes = remaining.allPanes()
            if remainingPanes.count == 1 {
                let survivingPane = remainingPanes[0]
                if survivingPane.id == tab.content.pane?.id {
                    tab.internalPanes = nil
                    tab.focusedPaneID = nil
                } else {
                    tab.internalPanes = .pane(survivingPane)
                    tab.focusedPaneID = survivingPane.id
                }
            } else {
                tab.internalPanes = remaining
                if tab.focusedPaneID == paneID {
                    tab.focusedPaneID = remainingPanes.first?.id
                }
            }
        }

        return paneID
    }
}
