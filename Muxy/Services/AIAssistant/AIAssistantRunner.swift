import Foundation

enum AIAssistantRunnerError: Error, LocalizedError {
    case providerNotConfigured(String)
    case commandNotFound(String)
    case nonZeroExit(status: Int32, stderr: String)
    case emptyOutput
    case launchFailed(String)
    case parsingFailed(String)
    case cancelled

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
        case .cancelled: "Generation cancelled."
        }
    }
}

struct AIAssistantInvocation {
    enum Kind {
        case direct(executable: String, arguments: [String])
        case shell(commandLine: String)
    }

    let kind: Kind
    let displayName: String
}

enum AIAssistantRunner {
    private static let runQueue = DispatchQueue(
        label: "app.muxy.ai-assistant",
        qos: .userInitiated,
        attributes: .concurrent
    )

    private static let stderrDrainQueue = DispatchQueue(
        label: "app.muxy.ai-assistant-stderr",
        qos: .userInitiated,
        attributes: .concurrent
    )

    private static let shellPathSearch = [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/usr/bin",
        "/bin",
    ]

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
            return AIAssistantInvocation(kind: .shell(commandLine: trimmed), displayName: firstToken(trimmed))
        }
        let executable = provider.defaultExecutable
        guard let resolved = resolveExecutable(executable) else {
            throw AIAssistantRunnerError.commandNotFound(executable)
        }
        return AIAssistantInvocation(
            kind: .direct(executable: resolved, arguments: provider.builtInArguments(model: model)),
            displayName: executable
        )
    }

    static func run(
        invocation: AIAssistantInvocation,
        prompt: String,
        workingDirectory: String
    ) async throws -> String {
        let handle = ProcessHandle()
        return try await withTaskCancellationHandler {
            try await dispatch {
                try executeSync(
                    invocation: invocation,
                    prompt: prompt,
                    workingDirectory: workingDirectory,
                    handle: handle
                )
            }
        } onCancel: {
            handle.terminate()
        }
    }

    private static func dispatch(
        _ work: @escaping @Sendable () throws -> String
    ) async throws -> String {
        try await withCheckedThrowingContinuation { continuation in
            runQueue.async {
                do {
                    try continuation.resume(returning: work())
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }

    private static func executeSync(
        invocation: AIAssistantInvocation,
        prompt: String,
        workingDirectory: String,
        handle: ProcessHandle
    ) throws -> String {
        let process = Process()
        switch invocation.kind {
        case let .direct(executable, arguments):
            process.executableURL = URL(fileURLWithPath: executable)
            process.arguments = arguments
            process.environment = enrichedEnvironment()
        case let .shell(commandLine):
            process.executableURL = URL(fileURLWithPath: userShell())
            process.arguments = ["-l", "-c", commandLine]
        }
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

        guard handle.attach(process) else {
            process.waitUntilExit()
            throw AIAssistantRunnerError.cancelled
        }
        defer { handle.detach() }

        let stderrCollector = AsyncDataCollector()
        stderrCollector.start(reading: stderrPipe.fileHandleForReading, on: stderrDrainQueue)

        if let data = prompt.data(using: .utf8) {
            stdinPipe.fileHandleForWriting.write(data)
        }
        try? stdinPipe.fileHandleForWriting.close()

        let stdoutData = stdoutPipe.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        let stderrData = stderrCollector.wait()

        if handle.wasCancelled {
            throw AIAssistantRunnerError.cancelled
        }

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

    private static func resolveExecutable(_ name: String) -> String? {
        if let direct = GitProcessRunner.resolveExecutable(name) {
            return direct
        }
        for directory in shellPathSearch {
            let path = "\(directory)/\(name)"
            if FileManager.default.isExecutableFile(atPath: path) {
                return path
            }
        }
        return resolveViaUserShell(name)
    }

    private static func resolveViaUserShell(_ name: String) -> String? {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: userShell())
        process.arguments = ["-l", "-c", "command -v \(name)"]
        let stdoutPipe = Pipe()
        process.standardOutput = stdoutPipe
        process.standardError = Pipe()
        do {
            try process.run()
        } catch {
            return nil
        }
        let data = stdoutPipe.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        guard process.terminationStatus == 0 else { return nil }
        let path = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        guard !path.isEmpty, FileManager.default.isExecutableFile(atPath: path) else { return nil }
        return path
    }

    private static func enrichedEnvironment() -> [String: String] {
        var environment = ProcessInfo.processInfo.environment
        let existing = environment["PATH"] ?? ""
        let extras = shellPathSearch.filter { !existing.contains($0) }
        if !extras.isEmpty {
            environment["PATH"] = (extras + [existing]).filter { !$0.isEmpty }.joined(separator: ":")
        }
        return environment
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

    private static func firstToken(_ command: String) -> String {
        command.split(whereSeparator: { $0.isWhitespace }).first.map(String.init) ?? command
    }
}

private final class ProcessHandle: @unchecked Sendable {
    private let lock = NSLock()
    private var process: Process?
    private var cancelled = false

    var wasCancelled: Bool {
        lock.lock()
        defer { lock.unlock() }
        return cancelled
    }

    func attach(_ process: Process) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        if cancelled {
            terminateRunning(process)
            return false
        }
        self.process = process
        return true
    }

    func detach() {
        lock.lock()
        defer { lock.unlock() }
        process = nil
    }

    func terminate() {
        lock.lock()
        defer { lock.unlock() }
        cancelled = true
        guard let process else { return }
        terminateRunning(process)
    }

    private func terminateRunning(_ process: Process) {
        guard process.isRunning else { return }
        process.terminate()
    }
}

private final class AsyncDataCollector: @unchecked Sendable {
    private let lock = NSLock()
    private var data = Data()
    private let semaphore = DispatchSemaphore(value: 0)

    func start(reading handle: FileHandle, on queue: DispatchQueue) {
        queue.async { [self] in
            let collected = handle.readDataToEndOfFile()
            lock.lock()
            data = collected
            lock.unlock()
            semaphore.signal()
        }
    }

    func wait() -> Data {
        semaphore.wait()
        lock.lock()
        defer { lock.unlock() }
        return data
    }
}
