import Foundation
import Testing

@testable import Muxy

@Suite("Hook session capture")
struct HookSessionCaptureTests {
    @Test("extracts session id and cwd from a SessionStart payload")
    func extractsFields() {
        let payload: [String: Any] = ["hook_event_name": "SessionStart",
                                      "session_id": "hook-sid", "cwd": "/Users/x/proj"]
        let parsed = HookSessionCapture.parse(payload: payload)
        #expect(parsed?.sessionID == "hook-sid")
        #expect(parsed?.cwd == "/Users/x/proj")
    }

    @Test("ignores non-session hook events")
    func ignoresOthers() {
        #expect(HookSessionCapture.parse(payload: ["hook_event_name": "Stop"]) == nil)
    }
}
