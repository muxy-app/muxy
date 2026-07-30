enum TerminalProcessClassifier {
    private static let shellProcessNames: Set<String> = [
        "bash",
        "csh",
        "dash",
        "fish",
        "ksh",
        "nu",
        "pwsh",
        "sh",
        "tcsh",
        "xonsh",
        "zsh",
    ]

    private static let terminalMultiplexerProcessNames: Set<String> = [
        "screen",
        "tmux",
        "zellij",
    ]

    static func isShell(_ processName: String?) -> Bool {
        guard let processName = normalizedProcessName(processName) else { return false }
        return shellProcessNames.contains(processName)
    }

    static func acceptsShellInput(_ processName: String?) -> Bool {
        guard let processName = normalizedProcessName(processName) else { return false }
        return shellProcessNames.contains(processName)
            || terminalMultiplexerProcessNames.contains(processName)
    }

    static func isInteractiveShell(processName: String?, arguments: [String]?) -> Bool {
        guard let processName = normalizedProcessName(processName),
              shellProcessNames.contains(processName),
              let arguments,
              !arguments.isEmpty
        else { return false }
        var shellArguments = Array(arguments.dropFirst())
        removeGhosttyBootstrapArguments(processName: processName, arguments: &shellArguments)
        return shellArguments.allSatisfy(isInteractiveShellOption)
    }

    private static func normalizedProcessName(_ processName: String?) -> String? {
        guard var processName = processName?.lowercased(), !processName.isEmpty else { return nil }
        if processName.hasPrefix("-") {
            processName.removeFirst()
        }
        return processName
    }

    private static func isInteractiveShellOption(_ argument: String) -> Bool {
        let argument = argument.lowercased()
        if argument == "--interactive" || argument == "--login" {
            return true
        }
        guard argument.hasPrefix("-"), argument.count > 1 else { return false }
        return argument.dropFirst().allSatisfy { $0 == "i" || $0 == "l" }
    }

    private static func removeGhosttyBootstrapArguments(
        processName: String,
        arguments: inout [String]
    ) {
        if processName == "bash", arguments.first == "--posix" {
            arguments.removeFirst()
        }
        if processName == "nu",
           arguments.starts(with: ["--execute", "use ghostty *"])
        {
            arguments.removeFirst(2)
        }
    }
}
