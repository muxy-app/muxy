import Foundation

enum TerminalOfflinePolicy {
    struct Candidate {
        let hasLiveSurface: Bool
        let isAlreadyOffline: Bool
        let invisibleDuration: TimeInterval?
        let isIdle: Bool
    }

    static func isIdle(hasRunningProcess: Bool, isAlternateScreen: Bool) -> Bool {
        !hasRunningProcess && !isAlternateScreen
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
