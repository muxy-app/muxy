import Darwin
import Foundation

enum LaunchArgumentGuard {
    static func isCLISubcommandLaunch(_ arguments: [String]) -> Bool {
        guard arguments.count > 1 else { return false }
        let first = arguments[1]
        if first.hasPrefix("-") { return false }
        if first.hasPrefix("/") || first.hasPrefix("~") { return false }
        return true
    }

    static func terminateIfNeeded(arguments: [String] = CommandLine.arguments) {
        guard isCLISubcommandLaunch(arguments) else { return }
        let message = "Error: Muxy app cannot run CLI subcommands directly. Install and use the muxy CLI wrapper.\n"
        FileHandle.standardError.write(Data(message.utf8))
        exit(EX_USAGE)
    }
}
