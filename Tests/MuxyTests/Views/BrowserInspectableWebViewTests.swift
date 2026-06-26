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
