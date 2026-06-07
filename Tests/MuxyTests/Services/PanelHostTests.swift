import Testing

@testable import Muxy

@MainActor
@Suite("PanelHost")
struct PanelHostTests {
    private func makeHost() -> PanelHost {
        let host = PanelHost.shared
        host.closeAll()
        return host
    }

    @Test("opening a panel records its placement")
    func opensPanel() {
        let host = makeHost()
        host.open("a", at: .right, mode: .pinned)
        #expect(host.isOpen("a"))
        #expect(host.pinnedPanel(at: .right) == "a")
    }

    @Test("only one pinned panel per position")
    func onePinnedPerPosition() {
        let host = makeHost()
        host.open("a", at: .right, mode: .pinned)
        host.open("b", at: .right, mode: .pinned)
        #expect(host.pinnedPanel(at: .right) == "b")
        #expect(!host.isOpen("a"))
    }

    @Test("only one floating panel per position")
    func oneFloatingPerPosition() {
        let host = makeHost()
        host.open("a", at: .bottom, mode: .floating)
        host.open("b", at: .bottom, mode: .floating)
        #expect(host.floatingPanel(at: .bottom) == "b")
        #expect(!host.isOpen("a"))
    }

    @Test("pinned and floating coexist at the same position")
    func pinnedAndFloatingCoexist() {
        let host = makeHost()
        host.open("pinned", at: .right, mode: .pinned)
        host.open("floating", at: .right, mode: .floating)
        #expect(host.pinnedPanel(at: .right) == "pinned")
        #expect(host.floatingPanel(at: .right) == "floating")
    }

    @Test("a panel opened twice keeps a single placement")
    func reopenMovesPanel() {
        let host = makeHost()
        host.open("a", at: .right, mode: .pinned)
        host.open("a", at: .bottom, mode: .floating)
        #expect(host.pinnedPanel(at: .right) == nil)
        #expect(host.floatingPanel(at: .bottom) == "a")
        #expect(host.placements.count == 1)
    }

    @Test("toggle opens then closes the same panel")
    func toggle() {
        let host = makeHost()
        host.toggle("a", at: .right, mode: .pinned)
        #expect(host.isOpen("a"))
        host.toggle("a", at: .right, mode: .pinned)
        #expect(!host.isOpen("a"))
    }

    @Test("move preserves mode")
    func movePreservesMode() {
        let host = makeHost()
        host.open("a", at: .right, mode: .floating)
        host.move("a", to: .bottom)
        #expect(host.placement(for: "a")?.position == .bottom)
        #expect(host.placement(for: "a")?.mode == .floating)
    }

    @Test("setMode preserves position and displaces same-mode panel")
    func setMode() {
        let host = makeHost()
        host.open("a", at: .right, mode: .pinned)
        host.open("b", at: .right, mode: .floating)
        host.setMode(.pinned, for: "b")
        #expect(host.placement(for: "b")?.mode == .pinned)
        #expect(host.placement(for: "b")?.position == .right)
        #expect(!host.isOpen("a"))
    }

    @Test("opening over an occupied slot reports the displaced panel")
    func displaceNotifiesEvictedPanel() {
        let host = makeHost()
        let previous = host.onDisplace
        defer { host.onDisplace = previous }
        var displaced: [String] = []
        host.onDisplace = { displaced.append($0) }

        host.open("a", at: .right, mode: .floating)
        host.open("b", at: .right, mode: .floating)
        #expect(displaced == ["a"])

        host.move("b", to: .right)
        #expect(displaced == ["a"])
    }
}
