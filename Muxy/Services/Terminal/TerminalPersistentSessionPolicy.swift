import Foundation

enum TerminalPersistentSessionPolicy {
    static func usesPersistentSession(
        preferenceEnabled: Bool,
        workspaceContext: WorkspaceContext,
        isAvailable: Bool
    ) -> Bool {
        guard preferenceEnabled, isAvailable else { return false }
        return !workspaceContext.isRemote
    }

    @MainActor
    static func usesPersistentSession(workspaceContext: WorkspaceContext) -> Bool {
        usesPersistentSession(
            preferenceEnabled: TerminalPersistentSessionPreferences.isEnabled,
            workspaceContext: workspaceContext,
            isAvailable: PersistentSessionService.shared.isAvailable
        )
    }

    static func isIdle(
        activity: SessionCommandActivity?,
        isShellCommandRunning: Bool,
        isAlternateScreen: Bool
    ) -> Bool {
        guard let activity else { return false }
        return TerminalOfflinePolicy.isIdle(
            hasRunningProcess: activity == .running || isShellCommandRunning,
            isAlternateScreen: isAlternateScreen
        )
    }
}
