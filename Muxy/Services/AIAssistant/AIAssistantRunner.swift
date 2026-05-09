import Foundation

enum AIAssistantRunnerError: Error, LocalizedError {
    case providerNotConfigured(String)
    case commandNotFound(String)
    case nonZeroExit(status: Int32, stderr: String)
    case emptyOutput
    case launchFailed(String)
    case parsingFailed(String)

    var errorDescription: String? {
        switch self {
        case let .providerNotConfigured(message): message
        case let .commandNotFound(name):
            "Could not run \(name). Make sure it is installed and available in your shell's PATH."
        case let .nonZeroExit(status, stderr):
            stderr.isEmpty ? "Provider exited with status \(status)." : stderr
        case .emptyOutput: "Provider returned an empty response."
        case let .launchFailed(message): "Failed to start provider: \(message)"
        case let .parsingFailed(message): message
        }
    }
}

struct AIAssistantInvocation {
    let executable: String
    let arguments: [String]
    let displayName: String
    let usesLoginShell: Bool
}

enum AIAssistantRunner {
    static func resolveInvocation(
        provider: AIAssistantProvider,
        customCommand: String,
        model: String?
    ) throws -> AIAssistantInvocation {
        if provider == .custom {
            let trimmed = customCommand.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else {
                throw AIAssistantRunnerError.providerNotConfigured(
                    "Custom command is empty. Configure it in Settings → AI."
                )
            }
            return AIAssistantInvocation(
                executable: userShell(),
                arguments: ["-l", "-c", trimmed],
                displayName: firstToken(trimmed),
                usesLoginShell: true
            )
        }
        let name = provider.defaultExecutable
        guard let resolved = GitProcessRunner.resolveExecutable(name) else {
            throw AIAssistantRunnerError.commandNotFound(name)
        }
        return AIAssistantInvocation(
            executable: resolved,
            arguments: provider.builtInArguments(model: model),
            displayName: name,
            usesLoginShell: false
        )
    }

    static func run(
        invocation: AIAssistantInvocation,
        prompt: String,
        workingDirectory: String
    ) async throws -> String {
        let stdin = prompt.data(using: .utf8) ?? Data()
        let result: GitProcessResult
        do {
            result = try await GitProcessRunner.runCommand(
                executable: invocation.executable,
                arguments: invocation.arguments,
                workingDirectory: workingDirectory,
                stdin: stdin
            )
        } catch let GitProcessError.launchFailed(message) {
            throw AIAssistantRunnerError.launchFailed(message)
        }
        if result.status == 127 || isCommandNotFound(stderr: result.stderr) {
            throw AIAssistantRunnerError.commandNotFound(invocation.displayName)
        }
        if result.status != 0 {
            throw AIAssistantRunnerError.nonZeroExit(status: result.status, stderr: result.stderr)
        }
        let trimmed = result.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty {
            throw AIAssistantRunnerError.emptyOutput
        }
        return trimmed
    }

    static func isCommandNotFound(stderr: String) -> Bool {
        let lowered = stderr.lowercased()
        return lowered.contains("command not found")
            || lowered.contains("no such file or directory")
    }

    private static func userShell() -> String {
        if let shell = ProcessInfo.processInfo.environment["SHELL"], !shell.isEmpty {
            return shell
        }
        guard let pw = getpwuid(getuid()), let shellPtr = pw.pointee.pw_shell else {
            return "/bin/zsh"
        }
        return String(cString: shellPtr)
    }

    static func firstToken(_ command: String) -> String {
        command.split(whereSeparator: { $0.isWhitespace }).first.map(String.init) ?? command
    }
}
