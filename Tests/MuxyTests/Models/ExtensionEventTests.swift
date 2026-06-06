import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionEvent")
struct ExtensionEventTests {
    @Test("serializes event with sorted payload keys")
    func serializesPayload() {
        let event = ExtensionEvent(
            name: "pane.created",
            payload: ["paneID": "abc", "areaID": "def"]
        )
        #expect(event.serialize() == "event|pane.created|areaID=def|paneID=abc")
    }

    @Test("strips pipe and newline from payload values")
    func sanitizesPayload() {
        let event = ExtensionEvent(
            name: "tab.focused",
            payload: ["title": "hello|world\nnext"]
        )
        #expect(event.serialize() == "event|tab.focused|title=hello world next")
    }

    @Test("serializes without payload")
    func noPayload() {
        let event = ExtensionEvent(name: "test")
        #expect(event.serialize() == "event|test")
    }

    @Test("defines lifecycle event names")
    func lifecycleEventNames() {
        #expect(ExtensionEventName.tabClosed == "tab.closed")
        #expect(ExtensionEventName.panelOpened == "panel.opened")
        #expect(ExtensionEventName.panelClosed == "panel.closed")
        #expect(ExtensionEventName.popoverOpened == "popover.opened")
        #expect(ExtensionEventName.popoverClosed == "popover.closed")
    }

    @Test("registers lifecycle verbs without a required permission")
    func lifecycleVerbsAreUngated() {
        #expect(MuxyAPI.Permissions.verbNames.contains("lifecycle.resolveBeforeClose"))
        #expect(MuxyAPI.Permissions.verbNames.contains("lifecycle.closeSelf"))
        #expect(MuxyAPI.Permissions.required(for: "lifecycle.resolveBeforeClose") == nil)
        #expect(MuxyAPI.Permissions.required(for: "lifecycle.closeSelf") == nil)
    }
}
