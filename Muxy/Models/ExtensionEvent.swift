import Foundation

struct ExtensionEvent: Equatable {
    let name: String
    let payload: [String: String]

    init(name: String, payload: [String: String] = [:]) {
        self.name = name
        self.payload = payload
    }

    func serialize() -> String {
        var line = "event|\(name)"
        for key in payload.keys.sorted() {
            let value = payload[key] ?? ""
            let sanitizedKey = key.replacingOccurrences(of: "|", with: "_").replacingOccurrences(of: "=", with: "_")
            let sanitizedValue = value
                .replacingOccurrences(of: "|", with: " ")
                .replacingOccurrences(of: "\n", with: " ")
            line += "|\(sanitizedKey)=\(sanitizedValue)"
        }
        return line
    }
}

enum ExtensionEventName {
    static let paneCreated = "pane.created"
    static let paneClosed = "pane.closed"
    static let paneFocused = "pane.focused"
    static let tabCreated = "tab.created"
    static let tabFocused = "tab.focused"
    static let projectSwitched = "project.switched"
    static let worktreeSwitched = "worktree.switched"
    static let notificationPosted = "notification.posted"
}
