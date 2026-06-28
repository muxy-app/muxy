import Foundation

struct TerminalCommandTrackingInputContext {
    let altScreen: Bool
    let foregroundProcessName: String?
}

enum TerminalCommandTrackingInputGate {
    private static let shellProcessNames: Set<String> = [
        "bash",
        "dash",
        "fish",
        "ksh",
        "nu",
        "pwsh",
        "sh",
        "screen",
        "tcsh",
        "tmux",
        "xonsh",
        "zellij",
        "zsh",
    ]

    static func shouldRecordInput(_ context: TerminalCommandTrackingInputContext) -> Bool {
        guard !context.altScreen else { return false }
        guard let processName = context.foregroundProcessName?.lowercased(), !processName.isEmpty else { return true }
        return shellProcessNames.contains(processName)
    }
}
