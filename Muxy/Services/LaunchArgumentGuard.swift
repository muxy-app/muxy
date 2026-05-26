import Darwin
import Foundation

enum LaunchArgumentGuard {
    private static let cliSubcommands: Set<String> = [
        "split-right", "split-down", "send", "send-keys", "read-screen",
        "close-pane", "rename-pane", "list-panes", "list-projects",
        "switch-project", "list-worktrees", "switch-worktree", "refresh-worktrees",
        "list-tabs", "switch-tab", "new-tab", "next-tab", "previous-tab",
    ]

    static func isCLISubcommandLaunch(_ arguments: [String]) -> Bool {
        guard arguments.count > 1 else { return false }
        return cliSubcommands.contains(arguments[1])
    }

    static func terminateIfNeeded(arguments: [String] = CommandLine.arguments) -> Never? {
        guard isCLISubcommandLaunch(arguments) else { return nil }
        let message = "Error: Muxy app cannot run CLI subcommands directly. Install and use the muxy CLI wrapper.\n"
        FileHandle.standardError.write(Data(message.utf8))
        exit(EX_USAGE)
    }
}
