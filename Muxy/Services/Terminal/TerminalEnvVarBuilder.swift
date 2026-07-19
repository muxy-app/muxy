import Foundation

@MainActor
enum TerminalEnvVarBuilder {
    static func build(paneID: UUID, worktreeKey key: WorktreeKey) -> [(key: String, value: String)] {
        [
            (key: "MUXY_PANE_ID", value: paneID.uuidString),
            (key: "MUXY_PROJECT_ID", value: key.projectID.uuidString),
            (key: "MUXY_WORKTREE_ID", value: key.worktreeID.uuidString),
            (key: "MUXY_SOCKET_PATH", value: NotificationSocketServer.socketPath),
            (key: "MUXY_HOOK_BIN", value: MuxyNotificationHooks.hookBinaryPath),
            (
                key: "MUXY_HOOK_SCRIPT",
                value: MuxyNotificationHooks.stagedScriptPath(named: "muxy-claude-hook", extension: "sh")
            ),
        ]
    }
}
