import Foundation

struct GitProcessResult {
    let status: Int32
    let stdout: String
    let stdoutData: Data
    let stderr: String
    let truncated: Bool
}

enum GitProcessError: Error {
    case launchFailed(String)
}

enum GitProcessRunner {
    private static let queue = DispatchQueue(
        label: "app.muxy.git-runner",
        qos: .userInitiated,
        attributes: .concurrent
    )

    private static let stderrDrainQueue = DispatchQueue(
        label: "app.muxy.git-stderr-drain",
        qos: .userInitiated,
        attributes: .concurrent
    )

    private static let searchPaths = [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/usr/bin",
        "/bin",
        "/usr/sbin",
        "/sbin",
    ]

    static func resolveExecutable(_ name: String) -> String? {
        for directory in searchPaths {
            let path = "\(directory)/\(name)"
            if FileManager.default.isExecutableFile(atPath: path) {
                return path
            }
        }
        return nil
    }

    private struct ProcessSpec {
        let executable: String
        let arguments: [String]
        let workingDirectory: String?
        let lineLimit: Int?
        var outputByteLimit: Int?
        let signpostName: StaticString
        var stdinData: Data?
    }

    static func runGit(
        repoPath: String,
        arguments: [String],
        lineLimit: Int? = nil,
        outputByteLimit: Int? = nil,
        context: WorkspaceContext = .local
    ) async throws -> GitProcessResult {
        guard case let .ssh(destination) = context else {
            return try await runProcess(
                ProcessSpec(
                    executable: "/usr/bin/env",
                    arguments: ["git"] + gitHubCredentialHelperArgs() + ["-C", repoPath] + arguments,
                    workingDirectory: nil,
                    lineLimit: lineLimit,
                    outputByteLimit: outputByteLimit,
                    signpostName: "git"
                )
            )
        }
        let resolved = CommandTransform.resolve(
            executable: "git",
            arguments: ["-C", repoPath] + arguments,
            workingDirectory: nil,
            in: .ssh(destination)
        )
        return try await SSHCommandRunner.withTimeout(SSHCommandRunner.defaultTimeout) {
            try await runProcess(
                ProcessSpec(
                    executable: resolved.executable,
                    arguments: resolved.arguments,
                    workingDirectory: resolved.workingDirectory,
                    lineLimit: lineLimit,
                    outputByteLimit: outputByteLimit,
                    signpostName: "git"
                )
            )
        }
    }

    static func gitHubCredentialHelperArgs(ghResolver: (String) -> String? = resolveExecutable) -> [String] {
        guard let ghPath = ghResolver("gh") else { return [] }
        return [
            "-c", "credential.helper=",
            "-c", "credential.https://github.com.helper=!\(ghPath) auth git-credential",
        ]
    }

    static func processEnvironment(_ base: [String: String] = ProcessInfo.processInfo.environment) -> [String: String] {
        var environment = base
        environment["GIT_OPTIONAL_LOCKS"] = "0"
        environment["PATH"] = pathValue(base["PATH"])
        return environment
    }

    private static func pathValue(_ currentPath: String?) -> String {
        let currentPaths = (currentPath ?? "")
            .split(separator: ":")
            .map(String.init)
            .filter { !$0.isEmpty }
        let paths = (currentPaths + searchPaths).reduce(into: [String]()) { result, path in
            if !result.contains(path) {
                result.append(path)
            }
        }
        return paths.joined(separator: ":")
    }

    static func runCommand(
        executable: String,
        arguments: [String],
        workingDirectory: String
    ) async throws -> GitProcessResult {
        try await runProcess(
            ProcessSpec(
                executable: executable,
                arguments: arguments,
                workingDirectory: workingDirectory,
                lineLimit: nil,
                signpostName: "command"
            )
        )
    }

    static func runResolved(
        _ resolved: ResolvedLaunch,
        lineLimit: Int? = nil,
        stdinData: Data? = nil,
        outputByteLimit: Int? = nil
    ) async throws -> GitProcessResult {
        try await runProcess(
            ProcessSpec(
                executable: resolved.executable,
                arguments: resolved.arguments,
                workingDirectory: resolved.workingDirectory,
                lineLimit: lineLimit,
                outputByteLimit: outputByteLimit,
                signpostName: "command",
                stdinData: stdinData
            )
        )
    }

    private static func runProcess(_ spec: ProcessSpec) async throws -> GitProcessResult {
        let handle = ProcessHandle()
        return try await withTaskCancellationHandler {
            try await dispatch {
                try runProcessSync(spec, handle: handle)
            }
        } onCancel: {
            handle.terminate()
        }
    }

    private static func dispatch(
        _ work: @escaping @Sendable () throws -> GitProcessResult
    ) async throws -> GitProcessResult {
        try await withCheckedThrowingContinuation { continuation in
            queue.async {
                do {
                    let result = try work()
                    continuation.resume(returning: result)
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }

    static func offMain<T: Sendable>(_ work: @escaping @Sendable () -> T) async -> T {
        await withCheckedContinuation { continuation in
            queue.async {
                continuation.resume(returning: work())
            }
        }
    }

    static func offMainThrowing<T: Sendable>(_ work: @escaping @Sendable () throws -> T) async throws -> T {
        try await withCheckedThrowingContinuation { continuation in
            queue.async {
                do {
                    try continuation.resume(returning: work())
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }

    private static func runProcessSync(
        _ spec: ProcessSpec,
        handle: ProcessHandle
    ) throws -> GitProcessResult {
        let signpostID = GitSignpost.begin(spec.signpostName, spec.arguments.prefix(3).joined(separator: " "))
        defer { GitSignpost.end(spec.signpostName, signpostID) }

        let process = Process()
        process.executableURL = URL(fileURLWithPath: spec.executable)
        process.arguments = spec.arguments

        process.environment = processEnvironment()

        if let workingDirectory = spec.workingDirectory {
            process.currentDirectoryURL = URL(fileURLWithPath: workingDirectory)
        }

        let stdoutPipe = Pipe()
        let stderrPipe = Pipe()
        process.standardOutput = stdoutPipe
        process.standardError = stderrPipe
        let stdinPipe = spec.stdinData.map { _ in Pipe() }
        if let stdinPipe {
            process.standardInput = stdinPipe
        }

        do {
            try process.run()
        } catch {
            throw GitProcessError.launchFailed(error.localizedDescription)
        }

        if let stdinPipe, let stdinData = spec.stdinData {
            let writer = stdinPipe.fileHandleForWriting
            try? writer.write(contentsOf: stdinData)
            try? writer.close()
        }

        guard handle.attach(process) else {
            process.waitUntilExit()
            return GitProcessResult(
                status: process.terminationStatus,
                stdout: "",
                stdoutData: Data(),
                stderr: "",
                truncated: true
            )
        }
        defer { handle.detach() }

        let stderrCollector = AsyncDataCollector()
        stderrCollector.start(
            reading: stderrPipe.fileHandleForReading,
            on: stderrDrainQueue,
            byteLimit: spec.outputByteLimit
        )

        let stdoutRead: OutputRead
        do {
            stdoutRead = try readStdout(
                handle: stdoutPipe.fileHandleForReading,
                process: process,
                lineLimit: spec.lineLimit,
                byteLimit: spec.outputByteLimit
            )
        } catch {
            handle.terminate()
            _ = stderrCollector.wait()
            process.waitUntilExit()
            throw error
        }

        process.waitUntilExit()
        let stderrRead = stderrCollector.wait()

        let stdout = String(data: stdoutRead.data, encoding: .utf8) ?? ""
        let stderr = String(data: stderrRead.data, encoding: .utf8) ?? ""
        let truncated = stdoutRead.truncated
            || stderrRead.truncated
            || process.terminationReason == .uncaughtSignal
        return GitProcessResult(
            status: process.terminationStatus,
            stdout: stdout,
            stdoutData: stdoutRead.data,
            stderr: stderr,
            truncated: truncated
        )
    }

    private static func readStdout(
        handle: FileHandle,
        process: Process,
        lineLimit: Int?,
        byteLimit: Int?
    ) throws -> OutputRead {
        guard let lineLimit else {
            return try readWithByteLimit(handle: handle, process: process, byteLimit: byteLimit)
        }
        return try readWithLineLimit(
            handle: handle,
            process: process,
            lineLimit: lineLimit,
            byteLimit: byteLimit
        )
    }

    private static func readWithByteLimit(
        handle: FileHandle,
        process: Process,
        byteLimit: Int?
    ) throws -> OutputRead {
        guard let byteLimit else {
            return OutputRead(data: handle.readDataToEndOfFile(), truncated: false)
        }

        var collected = Data()
        let chunkSize = 65536
        while true {
            let chunk = try handle.read(upToCount: chunkSize) ?? Data()
            if chunk.isEmpty {
                return OutputRead(data: collected, truncated: false)
            }
            let remaining = byteLimit - collected.count
            guard chunk.count <= remaining else {
                if remaining > 0 {
                    collected.append(chunk.prefix(remaining))
                }
                process.terminate()
                return OutputRead(data: collected, truncated: true)
            }
            collected.append(chunk)
        }
    }

    private static func readWithLineLimit(
        handle: FileHandle,
        process: Process,
        lineLimit: Int,
        byteLimit: Int?
    ) throws -> OutputRead {
        var collected = Data()
        var currentLineCount = 0
        let chunkSize = 65536

        while true {
            let chunk = try handle.read(upToCount: chunkSize) ?? Data()
            if chunk.isEmpty {
                return OutputRead(data: collected, truncated: false)
            }

            if let byteLimit {
                let remaining = byteLimit - collected.count
                guard chunk.count <= remaining else {
                    if remaining > 0 {
                        collected.append(chunk.prefix(remaining))
                    }
                    process.terminate()
                    return OutputRead(data: collected, truncated: true)
                }
            }
            collected.append(chunk)
            currentLineCount += chunk.reduce(into: 0) { count, byte in
                if byte == 0x0A {
                    count += 1
                }
            }

            if currentLineCount >= lineLimit {
                process.terminate()
                return OutputRead(data: collected, truncated: true)
            }
        }
    }
}

private struct OutputRead {
    let data: Data
    let truncated: Bool
}

private final class ProcessHandle: @unchecked Sendable {
    private let lock = NSLock()
    private var process: Process?
    private var cancelled = false

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
    private var truncated = false
    private let semaphore = DispatchSemaphore(value: 0)

    func start(reading handle: FileHandle, on queue: DispatchQueue, byteLimit: Int?) {
        queue.async { [self] in
            var collected = Data()
            var didTruncate = false
            while true {
                let chunk = (try? handle.read(upToCount: 65536)) ?? Data()
                if chunk.isEmpty {
                    break
                }
                guard !didTruncate else { continue }
                guard let byteLimit else {
                    collected.append(chunk)
                    continue
                }
                let remaining = byteLimit - collected.count
                if chunk.count <= remaining {
                    collected.append(chunk)
                    continue
                }
                if remaining > 0 {
                    collected.append(chunk.prefix(remaining))
                }
                didTruncate = true
            }
            lock.lock()
            data = collected
            truncated = didTruncate
            lock.unlock()
            semaphore.signal()
        }
    }

    func wait() -> OutputRead {
        semaphore.wait()
        lock.lock()
        defer { lock.unlock() }
        return OutputRead(data: data, truncated: truncated)
    }
}
