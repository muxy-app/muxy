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
    let commandLine: String
    let displayName: String
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
            return AIAssistantInvocation(commandLine: trimmed, displayName: firstToken(trimmed))
        }
        let parts = [provider.defaultExecutable] + provider.builtInArguments(model: model)
        let commandLine = parts.map(shellQuote).joined(separator: " ")
        return AIAssistantInvocation(commandLine: commandLine, displayName: provider.defaultExecutable)
    }

    static func run(
        invocation: AIAssistantInvocation,
        prompt: String,
        workingDirectory: String
    ) async throws -> String {
        try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                do {
                    let result = try executeSync(
                        invocation: invocation,
                        prompt: prompt,
                        workingDirectory: workingDirectory
                    )
                    continuation.resume(returning: result)
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }

    private static func executeSync(
        invocation: AIAssistantInvocation,
        prompt: String,
        workingDirectory: String
    ) throws -> String {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: userShell())
        process.arguments = ["-l", "-c", invocation.commandLine]
        process.currentDirectoryURL = URL(fileURLWithPath: workingDirectory)

        let stdinPipe = Pipe()
        let stdoutPipe = Pipe()
        let stderrPipe = Pipe()
        process.standardInput = stdinPipe
        process.standardOutput = stdoutPipe
        process.standardError = stderrPipe

        do {
            try process.run()
        } catch {
            throw AIAssistantRunnerError.launchFailed(error.localizedDescription)
        }

        if let data = prompt.data(using: .utf8) {
            stdinPipe.fileHandleForWriting.write(data)
        }
        try? stdinPipe.fileHandleForWriting.close()

        let stdoutData = stdoutPipe.fileHandleForReading.readDataToEndOfFile()
        let stderrData = stderrPipe.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()

        let stdout = String(data: stdoutData, encoding: .utf8) ?? ""
        let stderr = String(data: stderrData, encoding: .utf8) ?? ""

        if process.terminationStatus == 127 || stderr.contains("command not found") {
            throw AIAssistantRunnerError.commandNotFound(invocation.displayName)
        }
        if process.terminationStatus != 0 {
            throw AIAssistantRunnerError.nonZeroExit(status: process.terminationStatus, stderr: stderr)
        }
        let trimmed = stdout.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty {
            throw AIAssistantRunnerError.emptyOutput
        }
        return trimmed
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

    private static func shellQuote(_ value: String) -> String {
        if value.isEmpty { return "''" }
        let safe = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "@%+=:,./-_"))
        if value.unicodeScalars.allSatisfy({ safe.contains($0) }) {
            return value
        }
        let escaped = value.replacingOccurrences(of: "'", with: "'\\''")
        return "'\(escaped)'"
    }

    private static func firstToken(_ command: String) -> String {
        command.split(whereSeparator: { $0.isWhitespace }).first.map(String.init) ?? command
    }
}
