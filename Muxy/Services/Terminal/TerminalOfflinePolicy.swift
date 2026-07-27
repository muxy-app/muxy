import Foundation

enum TerminalOfflinePolicy {
    struct Candidate {
        let hasLiveSurface: Bool
        let isAlreadyOffline: Bool
        let invisibleDuration: TimeInterval?
        let isIdle: Bool
    }

    static let idleShellProcessNames: Set<String> = [
        "bash", "csh", "dash", "fish", "ksh", "nu", "pwsh", "sh", "tcsh", "zsh",
    ]

    static func isIdle(hasRunningProcess: Bool, isAlternateScreen: Bool) -> Bool {
        !hasRunningProcess && !isAlternateScreen
    }

    static func hasRunningProcess(foregroundProcessName: String?) -> Bool {
        guard var name = foregroundProcessName?.lowercased(), !name.isEmpty else { return true }
        if name.hasPrefix("-") {
            name.removeFirst()
        }
        return !idleShellProcessNames.contains(name)
    }

    static func keepsAwake(isOnScreen: Bool, isFocused: Bool) -> Bool {
        isOnScreen && isFocused
    }

    static func shouldTakeOffline(
        _ candidate: Candidate,
        isEnabled: Bool,
        idleThreshold: TimeInterval
    ) -> Bool {
        guard isEnabled, candidate.hasLiveSurface, !candidate.isAlreadyOffline else { return false }
        guard let invisibleDuration = candidate.invisibleDuration, invisibleDuration >= idleThreshold else {
            return false
        }
        return candidate.isIdle
    }
}

enum SleepingTabPlaceholderPolicy {
    static func shouldPresent(isVisible: Bool, isOffline: Bool, isRemotelyOwned: Bool) -> Bool {
        guard isVisible, isOffline, !isRemotelyOwned else { return false }
        return true
    }
}
