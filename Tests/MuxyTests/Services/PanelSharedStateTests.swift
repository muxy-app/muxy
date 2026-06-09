import Foundation
import Testing

@testable import Muxy

@Suite("Panel shared state", .serialized)
struct PanelSharedStateTests {
    @Suite("PanelHost")
    @MainActor
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

    @Suite("ExtensionPanelRegistry")
    @MainActor
    struct ExtensionPanelRegistryTests {
        @Test("displacing a panel at the same slot emits panel.closed for the displaced panel")
        func displacementEmitsPanelClosed() async {
            let registry = ExtensionPanelRegistry.shared
            registry.closeAll(extensionID: "ext-a")
            registry.closeAll(extensionID: "ext-b")

            let collector = EventCollector()
            let token = NotificationSocketServer.shared.addInProcessObserver { collector.add($0) }
            defer { NotificationSocketServer.shared.removeInProcessObserver(token) }

            registry.open(extensionID: "ext-a", panel: panel(id: "first"), data: nil)
            registry.open(extensionID: "ext-b", panel: panel(id: "second"), data: nil)
            defer { registry.closeAll(extensionID: "ext-b") }

            let delivered = await waitFor(timeout: 2.0) {
                collector.closedPanelIDs(extensionID: "ext-a").contains("first")
            }
            #expect(delivered)
            #expect(!collector.closedPanelIDs(extensionID: "ext-b").contains("second"))
        }

        private func panel(id: String) -> ExtensionPanel {
            ExtensionPanel(id: id, entry: "index.html", position: .right, mode: .pinned)
        }

        private func waitFor(timeout: TimeInterval, condition: () -> Bool) async -> Bool {
            let deadline = Date().addingTimeInterval(timeout)
            while Date() < deadline {
                if condition() { return true }
                try? await Task.sleep(nanoseconds: 50_000_000)
            }
            return condition()
        }
    }
}

private final class EventCollector: @unchecked Sendable {
    private let lock = NSLock()
    private var events: [ExtensionEvent] = []

    func add(_ event: ExtensionEvent) {
        lock.lock()
        events.append(event)
        lock.unlock()
    }

    func closedPanelIDs(extensionID: String) -> [String] {
        lock.lock()
        defer { lock.unlock() }
        return events
            .filter { $0.name == ExtensionEventName.panelClosed && $0.payload["extensionID"] == extensionID }
            .compactMap { $0.payload["panelID"] }
    }
}
