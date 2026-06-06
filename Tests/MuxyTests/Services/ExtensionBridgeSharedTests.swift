import Foundation
import Testing

@testable import Muxy

@Suite("Extension bridge shared helpers")
@MainActor
struct ExtensionBridgeSharedTests {
    @Test("decodes extension-local events from bridge args")
    func decodesExtensionLocalEvent() throws {
        let event = try ExtensionBridgeShared.decodeExtensionLocalEvent(args: [
            "event": "extension.panel.request",
            "payload": ["count": 2],
        ])
        #expect(event.name == "extension.panel.request")

        let object = try JSONSerialization.jsonObject(with: event.payload) as? [String: Int]
        #expect(object?["count"] == 2)
    }

    @Test("rejects non-local event names")
    func rejectsNonLocalEventName() {
        #expect(throws: APIError.self) {
            _ = try ExtensionBridgeShared.decodeExtensionLocalEvent(args: [
                "event": "pane.created",
                "payload": NSNull(),
            ])
        }
    }
}
