public struct SessionShellInvocation: Equatable, Sendable {
    public let executable: String
    public let arguments: [String]
    public let environment: [SessionEnvironmentEntry]

    public init(executable: String, arguments: [String], environment: [SessionEnvironmentEntry]) {
        self.executable = executable
        self.arguments = arguments
        self.environment = environment
    }
}

public struct SessionEnvironment: Equatable, Sendable {
    private var keys: [String]
    private var values: [String: String]

    public init(_ entries: [SessionEnvironmentEntry]) {
        keys = []
        values = [:]
        for entry in entries {
            if values[entry.key] == nil {
                keys.append(entry.key)
            }
            values[entry.key] = entry.value
        }
    }

    public subscript(key: String) -> String? {
        get { values[key] }
        set {
            guard let newValue else {
                keys.removeAll { $0 == key }
                values.removeValue(forKey: key)
                return
            }
            if values[key] == nil {
                keys.append(key)
            }
            values[key] = newValue
        }
    }

    public var entries: [SessionEnvironmentEntry] {
        keys.compactMap { key in
            guard let value = values[key] else { return nil }
            return SessionEnvironmentEntry(key: key, value: value)
        }
    }
}

public enum SessionShellIntegration {
    public static let defaultShell = "/bin/zsh"
    private static let posixShell = "/bin/sh"
    private static let xdgFallbackDataDirectories = "/usr/local/share:/usr/share"

    public static func invocation(
        command: String,
        shell: String,
        resourcesDirectory: String,
        environment: [SessionEnvironmentEntry]
    ) -> SessionShellInvocation {
        let resolvedShell = shell.isEmpty ? defaultShell : shell
        let name = shellName(resolvedShell)
        var environment = SessionEnvironment(environment)
        var arguments = ["-" + name]

        if !resourcesDirectory.isEmpty {
            environment["GHOSTTY_RESOURCES_DIR"] = resourcesDirectory
            let root = resourcesDirectory + "/shell-integration"
            switch name {
            case "zsh":
                applyZshIntegration(root: root, environment: &environment)
            case "bash":
                applyBashIntegration(root: root, environment: &environment)
                if command.isEmpty {
                    arguments.append("--posix")
                }
            case "fish",
                 "elvish",
                 "nu":
                applyXDGIntegration(root: root, environment: &environment)
            default:
                break
            }
        }

        guard command.isEmpty else {
            return SessionShellInvocation(
                executable: posixShell,
                arguments: [posixShell, "-c", "exec " + command],
                environment: environment.entries
            )
        }

        return SessionShellInvocation(
            executable: resolvedShell,
            arguments: arguments,
            environment: environment.entries
        )
    }

    public static func shellName(_ shell: String) -> String {
        guard let separator = shell.lastIndex(of: "/") else { return shell }
        return String(shell[shell.index(after: separator)...])
    }

    private static func applyZshIntegration(root: String, environment: inout SessionEnvironment) {
        if let existing = environment["ZDOTDIR"] {
            environment["GHOSTTY_ZSH_ZDOTDIR"] = existing
        }
        environment["ZDOTDIR"] = root + "/zsh"
    }

    private static func applyBashIntegration(root: String, environment: inout SessionEnvironment) {
        if let existing = environment["ENV"] {
            environment["GHOSTTY_BASH_ENV"] = existing
        }
        environment["ENV"] = root + "/bash/ghostty.bash"
        environment["GHOSTTY_BASH_INJECT"] = "1"
        if environment["HISTFILE"] == nil, let home = environment["HOME"], !home.isEmpty {
            environment["HISTFILE"] = home + "/.bash_history"
            environment["GHOSTTY_BASH_UNEXPORT_HISTFILE"] = "1"
        }
    }

    private static func applyXDGIntegration(root: String, environment: inout SessionEnvironment) {
        environment["GHOSTTY_SHELL_INTEGRATION_XDG_DIR"] = root
        guard let existing = environment["XDG_DATA_DIRS"], !existing.isEmpty else {
            environment["XDG_DATA_DIRS"] = root + ":" + xdgFallbackDataDirectories
            return
        }
        guard !existing.split(separator: ":").contains(where: { $0 == root }) else { return }
        environment["XDG_DATA_DIRS"] = root + ":" + existing
    }
}
