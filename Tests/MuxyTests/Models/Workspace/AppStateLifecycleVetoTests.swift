import Foundation
import Testing

@testable import Muxy

@Suite("AppState lifecycle veto", .serialized)
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

    @Test("Close Pane ignores a pinned active tab before requesting authorization")
    func closeAreaIgnoresPinnedActiveTab() async {
        let originalPreference = UserDefaults.standard.object(
            forKey: ProjectLifecyclePreferences.keepOpenWhenNoTabsKey
        )
        ProjectLifecyclePreferences.keepOpenWhenNoTabs = false
        defer {
            if let originalPreference {
                UserDefaults.standard.set(
                    originalPreference,
                    forKey: ProjectLifecyclePreferences.keepOpenWhenNoTabsKey
                )
            } else {
                UserDefaults.standard.removeObject(
                    forKey: ProjectLifecyclePreferences.keepOpenWhenNoTabsKey
                )
            }
        }

        let projectID = UUID()
        let worktreeID = UUID()
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let extensionState = ExtensionTabState(
            extensionID: "ext",
            tabTypeID: "editor",
            projectPath: "/tmp/test",
            defaultTitle: "Editor"
        )
        let tab = TerminalTab(extensionState: extensionState)
        let area = TabArea(projectPath: "/tmp/test", existingTab: tab)
        area.togglePin(tab.id)
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID
        appState.workspaceRoots[key] = .tabArea(area)
        appState.focusedAreaID[key] = area.id

        let surfaceKey = LifecycleSurfaceKey(kind: .tab, instanceID: extensionState.id.uuidString)
        let bridge = FakeBeforeCloseAsking(verdict: .allow)
        ExtensionSurfaceBridgeRegistry.shared.register(bridge, for: surfaceKey)
        defer { ExtensionSurfaceBridgeRegistry.shared.unregister(surfaceKey) }

        appState.closeArea(area.id, projectID: projectID)
        await settle()

        #expect(area.tabs.contains { $0.id == tab.id })
        #expect(bridge.askCount == 0)
        #expect(appState.pendingProcessTabClose == nil)
        #expect(appState.pendingLastTabClose == nil)
    }

    @Test("Close Pane lets a child extension veto closing its owner hierarchy")
    func childExtensionVetoesOwnerAreaClose() async {
        let projectID = UUID()
        let worktreeID = UUID()
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let rootArea = TabArea(projectPath: "/tmp/test")
        let rootTabID = rootArea.tabs[0].id
        let extensionState = ExtensionTabState(
            extensionID: "ext",
            tabTypeID: "editor",
            projectPath: "/tmp/test",
            defaultTitle: "Editor",
            data: nil
        )
        let childTab = TerminalTab(extensionState: extensionState, parentTabID: rootTabID)
        let childArea = TabArea(projectPath: "/tmp/test", existingTab: childTab)
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID
        appState.workspaceRoots[key] = .split(SplitBranch(
            direction: .horizontal,
            first: .tabArea(rootArea),
            second: .tabArea(childArea)
        ))
        appState.focusedAreaID[key] = rootArea.id

        let surfaceKey = LifecycleSurfaceKey(kind: .tab, instanceID: extensionState.id.uuidString)
        let bridge = FakeBeforeCloseAsking(verdict: .prevent)
        ExtensionSurfaceBridgeRegistry.shared.register(bridge, for: surfaceKey)
        defer { ExtensionSurfaceBridgeRegistry.shared.unregister(surfaceKey) }

        appState.closeArea(rootArea.id, projectID: projectID)
        await settle()

        #expect(appState.workspaceRoots[key]?.allTabs().count == 2)
        #expect(bridge.askCount == 1)
    }

    @Test("Close Pane asks every child extension before any verdict resolves")
    func ownerAreaCloseAsksChildExtensionsConcurrently() async {
        let projectID = UUID()
        let worktreeID = UUID()
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let rootArea = TabArea(projectPath: "/tmp/test")
        let rootTabID = rootArea.tabs[0].id
        let unrelatedTabID = rootArea.createTab()
        rootArea.activeTabID = rootTabID
        let firstState = ExtensionTabState(
            extensionID: "ext",
            tabTypeID: "first",
            projectPath: "/tmp/test",
            defaultTitle: "First",
            data: nil
        )
        let secondState = ExtensionTabState(
            extensionID: "ext",
            tabTypeID: "second",
            projectPath: "/tmp/test",
            defaultTitle: "Second",
            data: nil
        )
        let firstChild = TerminalTab(extensionState: firstState, parentTabID: rootTabID)
        let secondChild = TerminalTab(extensionState: secondState, parentTabID: rootTabID)
        let childArea = TabArea(projectPath: "/tmp/test", existingTab: firstChild)
        childArea.tabs.append(secondChild)
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID
        appState.workspaceRoots[key] = .split(SplitBranch(
            direction: .horizontal,
            first: .tabArea(rootArea),
            second: .tabArea(childArea)
        ))
        appState.focusedAreaID[key] = rootArea.id

        let firstKey = LifecycleSurfaceKey(kind: .tab, instanceID: firstState.id.uuidString)
        let secondKey = LifecycleSurfaceKey(kind: .tab, instanceID: secondState.id.uuidString)
        let firstBridge = DeferredBeforeCloseAsking()
        let secondBridge = DeferredBeforeCloseAsking()
        ExtensionSurfaceBridgeRegistry.shared.register(firstBridge, for: firstKey)
        ExtensionSurfaceBridgeRegistry.shared.register(secondBridge, for: secondKey)
        defer {
            ExtensionSurfaceBridgeRegistry.shared.unregister(firstKey)
            ExtensionSurfaceBridgeRegistry.shared.unregister(secondKey)
        }

        appState.closeArea(rootArea.id, projectID: projectID)
        await settle()

        let requestsStartedTogether = firstBridge.askCount == 1 && secondBridge.askCount == 1
        firstBridge.resolve(.allow)
        await settle()
        secondBridge.resolve(.allow)
        await settle()

        #expect(requestsStartedTogether)
        #expect(appState.workspaceRoots[key]?.allTabs().map(\.id) == [unrelatedTabID])
    }

    @Test("Close Pane asks for confirmation when a child terminal is running")
    func ownerAreaCloseConfirmsRunningChildTerminal() {
        let originalPreference = UserDefaults.standard.object(
            forKey: TabCloseConfirmationPreferences.confirmRunningProcessKey
        )
        TabCloseConfirmationPreferences.confirmRunningProcess = true
        defer {
            if let originalPreference {
                UserDefaults.standard.set(
                    originalPreference,
                    forKey: TabCloseConfirmationPreferences.confirmRunningProcessKey
                )
            } else {
                UserDefaults.standard.removeObject(
                    forKey: TabCloseConfirmationPreferences.confirmRunningProcessKey
                )
            }
        }

        let projectID = UUID()
        let worktreeID = UUID()
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let rootArea = TabArea(projectPath: "/tmp/test")
        let rootTabID = rootArea.tabs[0].id
        let childPane = TerminalPaneState(projectPath: "/tmp/test")
        let childTab = TerminalTab(pane: childPane, parentTabID: rootTabID)
        let childArea = TabArea(projectPath: "/tmp/test", existingTab: childTab)
        let terminalViews = TerminalViewRemovingStub(paneIDsRequiringConfirmation: [childPane.id])
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: terminalViews,
            workspacePersistence: WorkspacePersistenceStub()
        )
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID
        appState.workspaceRoots[key] = .split(SplitBranch(
            direction: .horizontal,
            first: .tabArea(rootArea),
            second: .tabArea(childArea)
        ))
        appState.focusedAreaID[key] = rootArea.id

        appState.closeArea(rootArea.id, projectID: projectID)

        #expect(appState.pendingProcessTabClose == AppState.PendingTabClose(
            key: key,
            areaID: rootArea.id,
            tabID: rootTabID
        ))
        #expect(appState.workspaceRoots[key]?.allTabs().count == 2)
        #expect(terminalViews.confirmationChecks.contains(childPane.id))
        appState.cancelCloseRunningTab()
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

@MainActor
private final class DeferredBeforeCloseAsking: BeforeCloseAsking {
    private(set) var askCount = 0
    private var continuation: CheckedContinuation<LifecycleVerdict, Never>?

    func requestBeforeClose(reason _: LifecycleSurfaceKind, instanceID _: String) async -> LifecycleVerdict {
        askCount += 1
        return await withCheckedContinuation { continuation = $0 }
    }

    func resolve(_ verdict: LifecycleVerdict) {
        let continuation = continuation
        self.continuation = nil
        continuation?.resume(returning: verdict)
    }

    func failPendingLifecycle() {
        resolve(.allow)
    }
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
    let paneIDsRequiringConfirmation: Set<UUID>
    private(set) var confirmationChecks: [UUID] = []

    init(paneIDsRequiringConfirmation: Set<UUID> = []) {
        self.paneIDsRequiringConfirmation = paneIDsRequiringConfirmation
    }

    func removeView(for paneID: UUID) {}

    func needsConfirmQuit(for paneID: UUID) -> Bool {
        confirmationChecks.append(paneID)
        return paneIDsRequiringConfirmation.contains(paneID)
    }
}
