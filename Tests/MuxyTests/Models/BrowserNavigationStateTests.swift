import Testing

@testable import Muxy

@Suite("BrowserNavigationState")
@MainActor
struct BrowserNavigationStateTests {
    @Test("init seeds pendingScrollRestore with current URL so restore only fires on the saved page")
    func pendingScrollRestoreCarriesURL() {
        let nav = BrowserNavigationState(
            initialURL: "https://example.com/article",
            scrollY: 420,
            zoom: 1.0
        )
        #expect(nav.pendingScrollRestore?.url == "https://example.com/article")
        #expect(nav.pendingScrollRestore?.y == 420)
    }

    @Test("init does not seed pendingScrollRestore when scroll position is zero")
    func noPendingRestoreWithoutScroll() {
        let nav = BrowserNavigationState(
            initialURL: "https://example.com",
            scrollY: 0,
            zoom: 1.0
        )
        #expect(nav.pendingScrollRestore == nil)
    }
}
