import Foundation

@MainActor
enum TerminalEnvVarBuilder {
    static func build(
        paneID: UUID,
        worktreeKey key: WorktreeKey,
        appSupportDirectory: URL = MuxyFileStorage.appSupportDirectory(),
        inheritedPath: String? = ProcessInfo.processInfo.environment["PATH"]
    ) -> [(key: String, value: String)] {
        var vars: [(key: String, value: String)] = [
            (key: "MUXY_PANE_ID", value: paneID.uuidString),
            (key: "MUXY_PROJECT_ID", value: key.projectID.uuidString),
            (key: "MUXY_WORKTREE_ID", value: key.worktreeID.uuidString),
            (key: "MUXY_SOCKET_PATH", value: NotificationSocketServer.socketPath),
        ]
        if let hookPath = MuxyNotificationHooks.hookScriptPath {
            vars.append((key: "MUXY_HOOK_SCRIPT", value: hookPath))
        }
        if let path = terminalPATH(
            appSupportDirectory: appSupportDirectory,
            inheritedPath: inheritedPath
        ) {
            vars.append((key: "PATH", value: path))
        }
        return vars
    }

    static func terminalPATH(
        appSupportDirectory: URL = MuxyFileStorage.appSupportDirectory(),
        inheritedPath: String? = ProcessInfo.processInfo.environment["PATH"]
    ) -> String? {
        guard let prefix = MuxyAgentBin.pathPrefixForTerminal(appSupportDirectory: appSupportDirectory) else {
            return nil
        }
        let base = inheritedPath ?? "/usr/bin:/bin:/usr/sbin:/sbin"
        return "\(prefix):\(base)"
    }
}
