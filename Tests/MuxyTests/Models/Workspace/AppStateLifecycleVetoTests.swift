import Foundation
import Testing

@testable import Muxy

@Suite("AppState lifecycle veto")
@MainActor
struct AppStateLifecycleVetoTests {
    @Test("an allowed verdict closes the extension tab")
    func allowClosesTab() async {
        let context = makeExtensionTabContext()
        let bridge = FakeBeforeCloseAsking(verdict: .allow)
        ExtensionSurfaceBridgeRegistry.shared.register(bridge, for: context.surfaceKey)
        defer { ExtensionSurfaceBridgeRegistry.shared.unregister(context.surfaceKey) }

        context.appState.closeTab(context.tabID, areaID: context.area.id, projectID: context.projectID)
        await settle()

        #expect(!context.area.tabs.contains { $0.id == context.tabID })
        #expect(bridge.askCount == 1)
    }

    @Test("a prevent verdict keeps the extension tab open")
    func preventKeepsTab() async {
        let context = makeExtensionTabContext()
        let bridge = FakeBeforeCloseAsking(verdict: .prevent)
        ExtensionSurfaceBridgeRegistry.shared.register(bridge, for: context.surfaceKey)
        defer { ExtensionSurfaceBridgeRegistry.shared.unregister(context.surfaceKey) }

        context.appState.closeTab(context.tabID, areaID: context.area.id, projectID: context.projectID)
        await settle()

        #expect(context.area.tabs.contains { $0.id == context.tabID })
        #expect(bridge.askCount == 1)
    }

    @Test("a terminal tab closes without asking any surface")
    func terminalTabSkipsGate() async {
        let context = makeExtensionTabContext()
        let area = context.area
        area.createTab()
        let terminalTabID = try! #require(area.tabs.last { $0.kind == .terminal }).id

        context.appState.closeTab(terminalTabID, areaID: area.id, projectID: context.projectID)
        await settle()

        #expect(!area.tabs.contains { $0.id == terminalTabID })
    }

    @Test("forceCloseTab(instanceID:) closes without asking the surface")
    func forceCloseByInstanceSkipsGate() async {
        let context = makeExtensionTabContext()
        let bridge = FakeBeforeCloseAsking(verdict: .prevent)
        ExtensionSurfaceBridgeRegistry.shared.register(bridge, for: context.surfaceKey)
        defer { ExtensionSurfaceBridgeRegistry.shared.unregister(context.surfaceKey) }

        context.appState.forceCloseTab(instanceID: context.surfaceKey.instanceID)
        await settle()

        #expect(!context.area.tabs.contains { $0.id == context.tabID })
        #expect(bridge.askCount == 0)
    }

    private func settle() async {
        for _ in 0 ..< 50 { await Task.yield() }
    }

    private struct Context {
        let appState: AppState
        let area: TabArea
        let projectID: UUID
        let tabID: UUID
        let surfaceKey: LifecycleSurfaceKey
    }

    private func makeExtensionTabContext() -> Context {
        let projectID = UUID()
        let worktreeID = UUID()
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = TabArea(projectPath: "/tmp/test")
        area.createExtensionTab(extensionID: "ext", tabTypeID: "editor", title: "Editor", data: nil)
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID
        appState.workspaceRoots[key] = .tabArea(area)
        appState.focusedAreaID[key] = area.id

        let tab = area.tabs.first { $0.kind == .extensionWebView }!
        let instanceID = tab.content.extensionState!.id.uuidString
        return Context(
            appState: appState,
            area: area,
            projectID: projectID,
            tabID: tab.id,
            surfaceKey: LifecycleSurfaceKey(kind: .tab, instanceID: instanceID)
        )
    }
}

@MainActor
private final class FakeBeforeCloseAsking: BeforeCloseAsking {
    let verdict: LifecycleVerdict
    private(set) var askCount = 0

    init(verdict: LifecycleVerdict) {
        self.verdict = verdict
    }

    func requestBeforeClose(reason _: LifecycleSurfaceKind, instanceID _: String) async -> LifecycleVerdict {
        askCount += 1
        return verdict
    }

    func failPendingLifecycle() {}
}

private final class WorkspacePersistenceStub: WorkspacePersisting {
    private var snapshots: [WorkspaceSnapshot] = []
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { snapshots }
    func saveWorkspaces(_ workspaces: [WorkspaceSnapshot]) throws { snapshots = workspaces }
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
    func removeView(for paneID: UUID) {}
    func needsConfirmQuit(for paneID: UUID) -> Bool { false }
}
