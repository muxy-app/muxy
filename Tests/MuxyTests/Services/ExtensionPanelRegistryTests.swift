import Foundation
import Testing

@testable import Muxy

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
