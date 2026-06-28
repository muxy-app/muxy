import Testing

@testable import Muxy

@MainActor
@Suite("PopoverHost")
struct PopoverHostTests {
    private func makeHost() -> PopoverHost {
        let host = PopoverHost.shared
        host.close()
        return host
    }

    private func popover(id: String = "p", width: Double = 320, height: Double = 360) -> ExtensionPopover {
        ExtensionPopover(id: id, entry: "popovers/\(id).html", width: width, height: height)
    }

    @Test("toggle opens then closes the same anchor")
    func toggleOpensAndCloses() {
        let host = makeHost()
        host.toggle(anchorID: "a", extensionID: "ext", popover: popover(), data: nil)
        #expect(host.isOpen(anchorID: "a"))
        host.toggle(anchorID: "a", extensionID: "ext", popover: popover(), data: nil)
        #expect(!host.isOpen(anchorID: "a"))
    }

    @Test("opening a second anchor closes the first")
    func singleOpenAtATime() {
        let host = makeHost()
        host.toggle(anchorID: "a", extensionID: "ext", popover: popover(), data: nil)
        host.toggle(anchorID: "b", extensionID: "ext", popover: popover(), data: nil)
        #expect(!host.isOpen(anchorID: "a"))
        #expect(host.isOpen(anchorID: "b"))
    }

    @Test("close by anchor only closes a matching anchor")
    func closeByAnchorMatches() {
        let host = makeHost()
        host.toggle(anchorID: "a", extensionID: "ext", popover: popover(), data: nil)
        host.close(anchorID: "other")
        #expect(host.isOpen(anchorID: "a"))
        host.close(anchorID: "a")
        #expect(!host.isOpen(anchorID: "a"))
    }

    @Test("close by extension only closes a matching extension")
    func closeByExtensionMatches() {
        let host = makeHost()
        host.toggle(anchorID: "a", extensionID: "ext", popover: popover(), data: nil)
        host.close(extensionID: "other")
        #expect(host.isOpen(anchorID: "a"))
        host.close(extensionID: "ext")
        #expect(!host.isOpen(anchorID: "a"))
    }

    @Test("resize clamps the reported size and is scoped to the extension")
    func resizeClampsAndScopes() {
        let host = makeHost()
        host.toggle(anchorID: "a", extensionID: "ext", popover: popover(), data: nil)

        host.resize(extensionID: "other", width: 100, height: 100)
        #expect(host.state(for: "ext")?.width == 320)

        host.resize(extensionID: "ext", width: 9999, height: 9999)
        #expect(host.state(for: "ext")?.width == PopoverHost.maxWidth)
        #expect(host.state(for: "ext")?.height == PopoverHost.maxHeight)

        host.resize(extensionID: "ext", width: 1, height: 1)
        #expect(host.state(for: "ext")?.width == PopoverHost.minSize)
        #expect(host.state(for: "ext")?.height == PopoverHost.minSize)
    }
}
