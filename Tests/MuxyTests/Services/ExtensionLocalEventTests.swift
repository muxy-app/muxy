import Foundation
import MuxyShared
import Testing

@testable import Muxy

@Suite("Extension local event transport")
struct ExtensionLocalEventTests {
    @Test("validates extension-local event names")
    func validatesNames() {
        #expect(ExtensionLocalEvent.isValidName("extension.panel.request"))
        #expect(ExtensionLocalEvent.isValidName("extension.panel:request"))
        #expect(!ExtensionLocalEvent.isValidName("pane.created"))
        #expect(!ExtensionLocalEvent.isValidName("extension:panel.request"))
        #expect(!ExtensionLocalEvent.isValidName("extension."))
        #expect(!ExtensionLocalEvent.isValidName("extension.bad name"))
    }

    @Test("serializes and parses base64 event envelopes")
    func serializesAndParsesEnvelope() throws {
        let payload = Data(#"{"text":"a|b\nc","count":3}"#.utf8)
        let line = try #require(ExtensionLocalEvent.serialize(name: "extension.panel.update", payload: payload))
        let parsed = try #require(ExtensionLocalEvent.parse(line))
        #expect(parsed.name == "extension.panel.update")
        #expect(parsed.payload == payload)
    }

    @Test("encodes JSON fragments and rejects oversize payloads")
    func encodesPayloads() throws {
        let stringPayload = try ExtensionLocalEvent.encodePayload("hello")
        #expect(String(data: stringPayload, encoding: .utf8) == #""hello""#)

        let objectPayload = try ExtensionLocalEvent.encodePayload(["ok": true])
        let object = try JSONSerialization.jsonObject(with: objectPayload) as? [String: Bool]
        #expect(object?["ok"] == true)

        #expect(throws: ExtensionLocalEvent.PayloadError.self) {
            _ = try ExtensionLocalEvent.encodePayload(String(repeating: "a", count: ExtensionLocalEvent.maxPayloadBytes + 1))
        }
    }

    @Test("only same-extension observers receive local events")
    func localEventDeliveryIsExtensionScoped() {
        #expect(NotificationSocketServer.canDeliverExtensionEventForTesting(
            observerExtensionID: "demo-a",
            incomingExtensionID: "demo-a"
        ))
        #expect(!NotificationSocketServer.canDeliverExtensionEventForTesting(
            observerExtensionID: "demo-b",
            incomingExtensionID: "demo-a"
        ))
    }
}
