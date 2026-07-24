import AppKit
import Testing
import WebKit

@testable import Muxy

@Suite("BrowserInspectableWebView")
@MainActor
struct BrowserInspectableWebViewTests {
    @Test("enables WebKit developer extras on browser configuration")
    func enablesDeveloperExtras() {
        let configuration = WKWebViewConfiguration()

        BrowserInspectableWebView.enableInspection(in: configuration)

        #expect(BrowserInspectableWebView.inspectionEnabled(in: configuration))
    }

    @Test("adds an enabled inspect element item to the web view menu")
    func addsInspectElementItem() {
        let webView = BrowserInspectableWebView(frame: .zero)
        webView.isInspectable = true
        let menu = NSMenu(title: "Browser")
        menu.addItem(withTitle: "Reload", action: nil, keyEquivalent: "")

        webView.addInspectElementItem(to: menu)
        webView.addInspectElementItem(to: menu)

        let inspectItems = menu.items.filter { $0.title == "Inspect Element" }
        #expect(inspectItems.count == 1)
        #expect(inspectItems.first?.isEnabled == true)
    }

    @Test("does not add inspect element when web view is not inspectable")
    func omitsInspectElementWhenNotInspectable() {
        let webView = BrowserInspectableWebView(frame: .zero)
        let menu = NSMenu(title: "Browser")

        webView.addInspectElementItem(to: menu)

        #expect(menu.items.isEmpty)
    }

    @Test("cached web view is not reused after profile changes")
    func cachedWebViewIsNotReusedAfterProfileChanges() {
        let firstProfileID = UUID()
        let secondProfileID = UUID()
        defer {
            BrowserDataStoreCache.shared.evict(firstProfileID)
            BrowserDataStoreCache.shared.evict(secondProfileID)
        }
        let state = BrowserTabState(projectPath: "/tmp/test", profileID: firstProfileID)
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = BrowserDataStoreCache.shared.store(for: firstProfileID)
        let webView = InspectorClosingWebViewStub(frame: .zero, configuration: configuration)
        state.webView = webView
        state.profileID = secondProfileID

        let reused = BrowserWebView.reusableWebView(
            for: state,
            dataStore: BrowserDataStoreCache.shared.store(for: secondProfileID)
        )

        #expect(reused == nil)
        #expect(state.webView == nil)
        #expect(webView.closeCount == 1)
    }

    @Test("cached web view is reused when the profile data store matches")
    func cachedWebViewIsReusedWhenDataStoreMatches() {
        let profileID = UUID()
        defer { BrowserDataStoreCache.shared.evict(profileID) }
        let dataStore = BrowserDataStoreCache.shared.store(for: profileID)
        let state = BrowserTabState(projectPath: "/tmp/test", profileID: profileID)
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = dataStore
        let webView = InspectorClosingWebViewStub(frame: .zero, configuration: configuration)
        state.webView = webView

        let reused = BrowserWebView.reusableWebView(for: state, dataStore: dataStore)

        #expect(reused === webView)
        #expect(state.webView === webView)
        #expect(webView.closeCount == 0)
    }

    @Test("registry unregisters only entries matching the retired web view")
    func registryUnregistersMatchingWebView() {
        let firstTabID = UUID()
        let secondTabID = UUID()
        let firstWebView = WKWebView(frame: .zero)
        let secondWebView = WKWebView(frame: .zero)
        BrowserWebViewRegistry.shared.register(firstWebView, for: firstTabID)
        BrowserWebViewRegistry.shared.register(secondWebView, for: secondTabID)
        defer {
            BrowserWebViewRegistry.shared.unregister(firstTabID)
            BrowserWebViewRegistry.shared.unregister(secondTabID)
        }

        BrowserWebViewRegistry.shared.unregister(firstWebView)

        #expect(BrowserWebViewRegistry.shared.webView(for: firstTabID) == nil)
        #expect(BrowserWebViewRegistry.shared.webView(for: secondTabID) === secondWebView)
    }

    @Test("cached web view remains registered while its tab state is inactive")
    func cachedWebViewRemainsRegisteredWhileInactive() async {
        var state: BrowserTabState? = BrowserTabState(projectPath: "/tmp/test")
        let webView = WKWebView(frame: .zero)
        let tabID = try? #require(state?.id)

        state?.webView = webView

        #expect(tabID.flatMap { BrowserWebViewRegistry.shared.webView(for: $0) } === webView)

        state = nil
        await Task.yield()

        #expect(tabID.flatMap { BrowserWebViewRegistry.shared.webView(for: $0) } == nil)
    }

    @Test("cached web view keeps its runtime attached while inactive")
    func cachedWebViewKeepsRuntimeAttachedWhileInactive() {
        let state = BrowserTabState(projectPath: "/tmp/test")
        let appState = AppState(
            selectionStore: BrowserSelectionStoreStub(),
            terminalViews: BrowserTerminalViewRemovingStub(),
            workspacePersistence: BrowserWorkspacePersistenceStub()
        )
        let historyStore = BrowserHistoryStore(persistence: InMemoryBrowserHistoryPersistence())
        let webView = WKWebView(frame: .zero)
        var coordinator: BrowserWebView.Coordinator? = BrowserWebView.Coordinator(
            state: state,
            appState: appState,
            historyStore: historyStore
        )
        state.webView = webView
        state.surfaceRuntime = coordinator
        coordinator?.attach(to: webView)
        let retainedCoordinator = coordinator

        coordinator = nil

        #expect(state.surfaceRuntime === retainedCoordinator)
        #expect(retainedCoordinator?.activeObservationCount == 6)
        #expect(webView.navigationDelegate === retainedCoordinator)
        #expect(webView.uiDelegate === retainedCoordinator)
    }

    @Test("cached web view handoff detaches the displaced coordinator")
    func cachedWebViewHandoffDetachesDisplacedCoordinator() {
        let state = BrowserTabState(projectPath: "/tmp/test")
        let appState = AppState(
            selectionStore: BrowserSelectionStoreStub(),
            terminalViews: BrowserTerminalViewRemovingStub(),
            workspacePersistence: BrowserWorkspacePersistenceStub()
        )
        let historyStore = BrowserHistoryStore(persistence: InMemoryBrowserHistoryPersistence())
        let source = BrowserWebView.Coordinator(
            state: state,
            appState: appState,
            historyStore: historyStore
        )
        let destination = BrowserWebView.Coordinator(
            state: state,
            appState: appState,
            historyStore: historyStore
        )
        let webView = WKWebView(frame: .zero)

        source.attach(to: webView)
        #expect(source.activeObservationCount == 6)

        destination.attach(to: webView)

        #expect(source.activeObservationCount == 0)
        #expect(destination.activeObservationCount == 6)
        #expect(webView.navigationDelegate === destination)
        #expect(webView.uiDelegate === destination)

        source.detach()

        #expect(destination.activeObservationCount == 6)
        #expect(webView.navigationDelegate === destination)
        #expect(webView.uiDelegate === destination)
    }
}

@MainActor
private final class InspectorClosingWebViewStub: WKWebView, BrowserElementInspecting {
    var closeCount = 0

    func inspectElement() -> Bool {
        true
    }

    func closeInspector() -> Bool {
        closeCount += 1
        return true
    }
}

@MainActor
private final class BrowserSelectionStoreStub: ActiveProjectSelectionStoring {
    func loadActiveProjectID() -> UUID? { nil }
    func saveActiveProjectID(_: UUID?) {}
    func loadActiveWorktreeIDs() -> [UUID: UUID] { [:] }
    func saveActiveWorktreeIDs(_: [UUID: UUID]) {}
}

@MainActor
private final class BrowserTerminalViewRemovingStub: TerminalViewRemoving {
    func removeView(for _: UUID) {}
    func needsConfirmQuit(for _: UUID) -> Bool { false }
}

private final class BrowserWorkspacePersistenceStub: WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { [] }
    func saveWorkspaces(_: [WorkspaceSnapshot]) throws {}
}
