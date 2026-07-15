import Foundation
import MuxyShared
import Testing

@testable import Muxy

@Suite("Remote workspace navigation")
struct RemoteWorkspaceNavigationTests {
    @Test("flattens split areas and resolves the focused area")
    func areaResolution() {
        let first = area(title: "One")
        let second = area(title: "Two")
        let workspace = WorkspaceDTO(
            projectID: UUID(),
            worktreeID: UUID(),
            focusedAreaID: second.id,
            root: .split(SplitBranchDTO(
                id: UUID(),
                direction: .horizontal,
                ratio: 0.4,
                first: .tabArea(first),
                second: .tabArea(second)
            ))
        )

        #expect(RemoteWorkspaceNavigation.areas(in: workspace.root).map(\.id) == [first.id, second.id])
        #expect(RemoteWorkspaceNavigation.focusedArea(in: workspace)?.id == second.id)
    }

    @Test("cycles tabs in both directions")
    func tabCycling() {
        let first = TabDTO(id: UUID(), kind: .terminal, title: "One", isPinned: false)
        let second = TabDTO(id: UUID(), kind: .terminal, title: "Two", isPinned: false)
        let area = TabAreaDTO(id: UUID(), projectPath: "/tmp", tabs: [first, second], activeTabID: first.id)

        #expect(RemoteWorkspaceNavigation.tab(in: area, offset: 1)?.id == second.id)
        #expect(RemoteWorkspaceNavigation.tab(in: area, offset: -1)?.id == second.id)
    }

    @Test("cycles tabs across split panes")
    func crossPaneTabCycling() {
        let first = area(title: "One")
        let second = area(title: "Two")
        let workspace = WorkspaceDTO(
            projectID: UUID(),
            worktreeID: UUID(),
            focusedAreaID: first.id,
            root: .split(SplitBranchDTO(
                id: UUID(),
                direction: .horizontal,
                ratio: 0.5,
                first: .tabArea(first),
                second: .tabArea(second)
            ))
        )

        let next = RemoteWorkspaceNavigation.tabAcrossAreas(in: workspace, offset: 1)
        let previous = RemoteWorkspaceNavigation.tabAcrossAreas(in: workspace, offset: -1)

        #expect(next?.area.id == second.id)
        #expect(next?.tab.id == second.activeTabID)
        #expect(previous?.area.id == second.id)
        #expect(previous?.tab.id == second.activeTabID)
    }

    private func area(title: String) -> TabAreaDTO {
        let tab = TabDTO(id: UUID(), kind: .terminal, title: title, isPinned: false)
        return TabAreaDTO(id: UUID(), projectPath: "/tmp", tabs: [tab], activeTabID: tab.id)
    }
}
