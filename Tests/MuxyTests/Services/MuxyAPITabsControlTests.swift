import Foundation
import Testing

@testable import Muxy

@Suite("MuxyAPI.Tabs control commands")
@MainActor
struct MuxyAPITabsControlTests {
    private let testPath = "/tmp/test"

    @Test("rename sets and resets the tab title")
    func renameSetsAndResets() {
        let (appState, area) = makeAppState(tabTitles: ["First", "Second"])
        let tab = area.tabs[1]

        _ = MuxyAPI.Tabs.rename(identifier: tab.id.uuidString, title: "Renamed", appState: appState)
        #expect(tab.customTitle == "Renamed")

        _ = MuxyAPI.Tabs.rename(identifier: tab.id.uuidString, title: "  ", appState: appState)
        #expect(tab.customTitle == nil)
    }

    @Test("rename resolves a tab by index")
    func renameResolvesByIndex() {
        let (appState, area) = makeAppState(tabTitles: ["First", "Second"])

        _ = MuxyAPI.Tabs.rename(identifier: "1", title: "ByIndex", appState: appState)
        #expect(area.tabs[1].customTitle == "ByIndex")
    }

    @Test("setColor validates against the palette and resets")
    func setColorValidatesAndResets() {
        let (appState, area) = makeAppState(tabTitles: ["First"])
        let tab = area.tabs[0]

        let ok = MuxyAPI.Tabs.setColor(identifier: "0", color: "blue", appState: appState)
        guard case .success = ok else { Issue.record("expected success"); return }
        #expect(tab.colorID == "blue")

        let bad = MuxyAPI.Tabs.setColor(identifier: "0", color: "not-a-color", appState: appState)
        guard case let .failure(error) = bad else { Issue.record("expected failure"); return }
        #expect(error == .invalidArguments("unknown color 'not-a-color'"))
        #expect(tab.colorID == "blue")

        _ = MuxyAPI.Tabs.setColor(identifier: "0", color: nil, appState: appState)
        #expect(tab.colorID == nil)
    }

    @Test("setIcon stores and resets the SF Symbol")
    func setIconStoresAndResets() {
        let (appState, area) = makeAppState(tabTitles: ["First"])
        let tab = area.tabs[0]

        _ = MuxyAPI.Tabs.setIcon(identifier: "0", icon: "flame.fill", appState: appState)
        #expect(tab.customIcon == "flame.fill")

        _ = MuxyAPI.Tabs.setIcon(identifier: "0", icon: " ", appState: appState)
        #expect(tab.customIcon == nil)
    }

    @Test("setPinned pins and unpins idempotently")
    func setPinnedTogglesIdempotently() {
        let (appState, area) = makeAppState(tabTitles: ["First", "Second"])
        let tab = area.tabs[1]

        _ = MuxyAPI.Tabs.setPinned(identifier: tab.id.uuidString, pinned: true, appState: appState)
        #expect(tab.isPinned)

        _ = MuxyAPI.Tabs.setPinned(identifier: tab.id.uuidString, pinned: true, appState: appState)
        #expect(tab.isPinned)

        _ = MuxyAPI.Tabs.setPinned(identifier: tab.id.uuidString, pinned: false, appState: appState)
        #expect(!tab.isPinned)
    }

    @Test("close removes an unpinned tab")
    func closeRemovesTab() {
        let (appState, area) = makeAppState(tabTitles: ["First", "Second"])
        let tab = area.tabs[1]

        _ = MuxyAPI.Tabs.close(identifier: tab.id.uuidString, appState: appState)
        #expect(!area.tabs.contains { $0.id == tab.id })
    }

    @Test("move reorders a tab within its area")
    func moveReordersTab() {
        let (appState, area) = makeAppState(tabTitles: ["First", "Second", "Third"])
        let third = area.tabs[2]

        _ = MuxyAPI.Tabs.move(identifier: third.id.uuidString, toIndex: 0, appState: appState)
        #expect(area.tabs.first?.id == third.id)
    }

    @Test("move rejects an out-of-range index")
    func moveRejectsOutOfRange() {
        let (appState, area) = makeAppState(tabTitles: ["First", "Second"])

        let result = MuxyAPI.Tabs.move(identifier: "0", toIndex: 5, appState: appState)
        guard case let .failure(error) = result else { Issue.record("expected failure"); return }
        #expect(error == .invalidArguments("index out of range"))
        #expect(area.tabs[0].title == "First")
    }

    @Test("move keeps an unpinned tab out of the pinned region")
    func moveRejectsCrossingPinnedBoundary() {
        let (appState, area) = makeAppState(tabTitles: ["Pinned", "First", "Second"])
        area.togglePin(area.tabs[0].id)
        let unpinned = area.tabs[2]

        let result = MuxyAPI.Tabs.move(identifier: unpinned.id.uuidString, toIndex: 0, appState: appState)
        guard case let .failure(error) = result else { Issue.record("expected failure"); return }
        #expect(error == .invalidArguments("index out of range"))
        #expect(area.tabs[0].isPinned)
        #expect(area.tabs.firstIndex(where: { !$0.isPinned }) == 1)
    }

    @Test("move reorders within the unpinned region while a pinned tab is present")
    func moveReordersWithinUnpinnedRegion() {
        let (appState, area) = makeAppState(tabTitles: ["Pinned", "First", "Second"])
        area.togglePin(area.tabs[0].id)
        let second = area.tabs[2]

        let result = MuxyAPI.Tabs.move(identifier: second.id.uuidString, toIndex: 1, appState: appState)
        guard case .success = result else { Issue.record("expected success"); return }
        #expect(area.tabs[0].isPinned)
        #expect(area.tabs[1].id == second.id)
    }

    @Test("a tab resolves by ID across a non-active workspace")
    func resolvesTabAcrossWorkspaces() {
        let (appState, _) = makeAppState(tabTitles: ["First"])
        let otherProjectID = UUID()
        let otherKey = WorktreeKey(projectID: otherProjectID, worktreeID: UUID())
        let otherArea = TabArea(projectPath: testPath)
        appState.workspaceRoots[otherKey] = .tabArea(otherArea)
        let target = otherArea.tabs[0]

        _ = MuxyAPI.Tabs.rename(identifier: target.id.uuidString, title: "Remote", appState: appState)
        #expect(target.customTitle == "Remote")
    }

    @Test("close resolves a tab by ID in a non-active worktree")
    func closeResolvesTabByIDInNonActiveWorktree() {
        let (appState, _) = makeAppState(tabTitles: ["First"])
        let projectID = appState.activeProjectID!
        let otherKey = WorktreeKey(projectID: projectID, worktreeID: UUID())
        let otherArea = TabArea(projectPath: testPath)
        let secondTabID = otherArea.createTab()
        appState.workspaceRoots[otherKey] = .tabArea(otherArea)
        appState.focusedAreaID[otherKey] = otherArea.id

        let result = MuxyAPI.Tabs.close(identifier: secondTabID.uuidString, appState: appState)

        guard case .success = result else { Issue.record("expected success"); return }
        #expect(!otherArea.tabs.contains { $0.id == secondTabID })
    }

    @Test("an unknown identifier fails with tabNotFound")
    func unknownIdentifierFails() {
        let (appState, _) = makeAppState(tabTitles: ["First"])
        let missing = UUID().uuidString

        let result = MuxyAPI.Tabs.rename(identifier: missing, title: "x", appState: appState)
        guard case let .failure(error) = result else { Issue.record("expected failure"); return }
        #expect(error == .tabNotFound(missing))
    }

    private func makeAppState(tabTitles: [String]) -> (AppState, TabArea) {
        let projectID = UUID()
        let worktreeID = UUID()
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = TabArea(projectPath: testPath)
        area.tabs[0].customTitle = tabTitles[0]
        for title in tabTitles.dropFirst() {
            let id = area.createTab()
            area.setCustomTitle(id, title: title)
        }
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID
        appState.workspaceRoots[key] = .tabArea(area)
        appState.focusedAreaID[key] = area.id
        return (appState, area)
    }
}

private final class WorkspacePersistenceStub: WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { [] }
    func saveWorkspaces(_: [WorkspaceSnapshot]) throws {}
}

@MainActor
private final class SelectionStoreStub: ActiveProjectSelectionStoring {
    private var activeProjectID: UUID?
    private var activeWorktreeIDs: [UUID: UUID] = [:]
    func loadActiveProjectID() -> UUID? { activeProjectID }
    func saveActiveProjectID(_ id: UUID?) { activeProjectID = id }
    func loadActiveWorktreeIDs() -> [UUID: UUID] { activeWorktreeIDs }
    func saveActiveWorktreeIDs(_ ids: [UUID: UUID]) { activeWorktreeIDs = ids }
}

@MainActor
private final class TerminalViewRemovingStub: TerminalViewRemoving {
    func removeView(for _: UUID) {}
    func needsConfirmQuit(for _: UUID) -> Bool { false }
}
