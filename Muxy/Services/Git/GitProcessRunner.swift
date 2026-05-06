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
        let signpostName: StaticString
    }

    static func runGit(
        repoPath: String,
        arguments: [String],
        lineLimit: Int? = nil
    ) async throws -> GitProcessResult {
        try await runProcess(
            ProcessSpec(
                executable: "/usr/bin/env",
                arguments: ["git", "-C", repoPath] + arguments,
                workingDirectory: nil,
                lineLimit: lineLimit,
                signpostName: "git"
            )
        )
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

        var environment = ProcessInfo.processInfo.environment
        environment["GIT_OPTIONAL_LOCKS"] = "0"
        process.environment = environment

        if let workingDirectory = spec.workingDirectory {
            process.currentDirectoryURL = URL(fileURLWithPath: workingDirectory)
        }

        let stdoutPipe = Pipe()
        let stderrPipe = Pipe()
        process.standardOutput = stdoutPipe
        process.standardError = stderrPipe

        do {
            try process.run()
        } catch {
            throw GitProcessError.launchFailed(error.localizedDescription)
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
        stderrCollector.start(reading: stderrPipe.fileHandleForReading, on: stderrDrainQueue)

        let stdoutData: Data
        do {
            stdoutData = try readStdout(
                handle: stdoutPipe.fileHandleForReading,
                process: process,
                lineLimit: spec.lineLimit
            )
        } catch {
            handle.terminate()
            _ = stderrCollector.wait()
            process.waitUntilExit()
            throw error
        }

        process.waitUntilExit()
        let stderrData = stderrCollector.wait()

        let stdout = String(data: stdoutData, encoding: .utf8) ?? ""
        let stderr = String(data: stderrData, encoding: .utf8) ?? ""
        let truncated = process.terminationReason == .uncaughtSignal
        return GitProcessResult(
            status: process.terminationStatus,
            stdout: stdout,
            stdoutData: stdoutData,
            stderr: stderr,
            truncated: truncated
        )
    }

    private static func readStdout(
        handle: FileHandle,
        process: Process,
        lineLimit: Int?
    ) throws -> Data {
        guard let lineLimit else {
            return handle.readDataToEndOfFile()
        }
        return try readWithLineLimit(handle: handle, process: process, lineLimit: lineLimit)
    }

    private static func readWithLineLimit(
        handle: FileHandle,
        process: Process,
        lineLimit: Int
    ) throws -> Data {
        var collected = Data()
        var currentLineCount = 0
        let chunkSize = 65536

        while true {
            let chunk = try handle.read(upToCount: chunkSize) ?? Data()
            if chunk.isEmpty {
                return collected
            }

            collected.append(chunk)
            currentLineCount += chunk.reduce(into: 0) { count, byte in
                if byte == 0x0A { count += 1 }
            }

            if currentLineCount >= lineLimit {
                process.terminate()
                return collected
            }
        }
    }
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
