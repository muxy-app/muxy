import Foundation

enum HookSessionCapture {
    struct Parsed: Equatable {
        let sessionID: String
        let cwd: String
    }

    static func parse(payload: [String: Any]) -> Parsed? {
        guard (payload["hook_event_name"] as? String) == "SessionStart",
              let sessionID = payload["session_id"] as? String,
              let cwd = payload["cwd"] as? String
        else { return nil }
        return Parsed(sessionID: sessionID, cwd: cwd)
    }
}
