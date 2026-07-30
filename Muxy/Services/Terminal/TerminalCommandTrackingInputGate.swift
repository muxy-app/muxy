import Foundation

struct TerminalCommandTrackingInputContext {
    let altScreen: Bool
    let foregroundProcessName: String?
}

enum TerminalCommandTrackingInputGate {
    static func shouldRecordInput(_ context: TerminalCommandTrackingInputContext) -> Bool {
        guard !context.altScreen else { return false }
        guard let processName = context.foregroundProcessName, !processName.isEmpty else { return true }
        return TerminalProcessClassifier.acceptsShellInput(processName)
    }
}
