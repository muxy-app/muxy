import Darwin
import Foundation

struct GitProcessResult {
    let status: Int32
    let stdout: String
    let stdoutData: Data
    let stderr: String
    let truncated: Bool
}

enum GitProcessError: LocalizedError {
    case launchFailed(String)
    case timedOut(TimeInterval)

    var errorDescription: String? {
        switch self {
        case let .launchFailed(message): message
        case let .timedOut(seconds): "Git timed out after \(Int(seconds))s"
        }
    }
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

    private static let stdinWriteQueue = DispatchQueue(
        label: "app.muxy.git-stdin-writer",
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
        context: WorkspaceContext = .local,
        timeout: TimeInterval? = nil
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
                ),
                timeout: timeout
            )
        }
        let resolved = CommandTransform.resolve(
            executable: "git",
            arguments: ["-C", repoPath] + arguments,
            workingDirectory: nil,
            in: .ssh(destination)
        )
        return try await SSHCommandRunner.withTimeout(timeout ?? SSHCommandRunner.defaultTimeout) {
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

    private static func runProcess(_ spec: ProcessSpec, timeout: TimeInterval? = nil) async throws -> GitProcessResult {
        guard let timeout else { return try await runProcessWithoutTimeout(spec) }
        do {
            return try await AsyncTimeout.run(seconds: timeout) {
                try await runProcessWithoutTimeout(spec)
            }
        } catch AsyncTimeoutError.timedOut {
            throw GitProcessError.timedOut(timeout)
        }
    }

    private static func runProcessWithoutTimeout(_ spec: ProcessSpec) async throws -> GitProcessResult {
        let handle = ProcessHandle()
        let result = try await withTaskCancellationHandler {
            try await dispatch {
                try runProcessSync(spec, handle: handle)
            }
        } onCancel: {
            handle.terminate()
        }
        try Task.checkCancellation()
        return result
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
        let stdinPipe = Pipe()
        process.standardInput = stdinPipe

        let exited = DispatchSemaphore(value: 0)
        let cancellableProcess: CancellableProcess
        do {
            cancellableProcess = try CancellableProcess.launch(
                configuredProcess: process,
                stdinPipe: stdinPipe,
                stdoutPipe: stdoutPipe,
                stderrPipe: stderrPipe,
                deferReaping: true
            ) {
                exited.signal()
            }
        } catch {
            throw GitProcessError.launchFailed(error.localizedDescription)
        }

        let stdinWriter = AsyncDataWriter(handle: stdinPipe.fileHandleForWriting)
        guard handle.attach(
            cancellableProcess,
            stdinWriter: stdinWriter
        )
        else {
            try? stdoutPipe.fileHandleForReading.close()
            try? stderrPipe.fileHandleForReading.close()
            cancellableProcess.releaseReaping()
            exited.wait()
            handle.detach()
            return GitProcessResult(
                status: cancellableProcess.terminationStatus,
                stdout: "",
                stdoutData: Data(),
                stderr: "",
                truncated: true
            )
        }
        defer { handle.detach() }
        configureNonblockingRead(stdoutPipe.fileHandleForReading)
        configureNonblockingRead(stderrPipe.fileHandleForReading)

        let stderrCollector = AsyncDataCollector()
        stderrCollector.start(
            reading: stderrPipe.fileHandleForReading,
            on: stderrDrainQueue,
            byteLimit: spec.outputByteLimit,
            processHandle: handle
        )

        stdinWriter.start(
            writing: spec.stdinData ?? Data(),
            on: stdinWriteQueue
        )

        let stdoutRead: OutputRead
        do {
            stdoutRead = try readStdout(
                handle: stdoutPipe.fileHandleForReading,
                processHandle: handle,
                lineLimit: spec.lineLimit,
                byteLimit: spec.outputByteLimit
            )
        } catch {
            handle.terminate()
            stdinWriter.waitUntilFinished()
            _ = stderrCollector.wait()
            try? stdoutPipe.fileHandleForReading.close()
            try? stderrPipe.fileHandleForReading.close()
            cancellableProcess.releaseReaping()
            exited.wait()
            throw error
        }

        stdinWriter.waitUntilFinished()
        let stderrRead = stderrCollector.wait()
        try? stdoutPipe.fileHandleForReading.close()
        try? stderrPipe.fileHandleForReading.close()
        cancellableProcess.releaseReaping()
        exited.wait()

        let stdout = String(data: stdoutRead.data, encoding: .utf8) ?? ""
        let stderr = String(data: stderrRead.data, encoding: .utf8) ?? ""
        let truncated = stdoutRead.truncated
            || stderrRead.truncated
            || handle.wasCancelled
        return GitProcessResult(
            status: cancellableProcess.terminationStatus,
            stdout: stdout,
            stdoutData: stdoutRead.data,
            stderr: stderr,
            truncated: truncated
        )
    }

    private static func readStdout(
        handle: FileHandle,
        processHandle: ProcessHandle,
        lineLimit: Int?,
        byteLimit: Int?
    ) throws -> OutputRead {
        guard let lineLimit else {
            return try readWithByteLimit(
                handle: handle,
                processHandle: processHandle,
                byteLimit: byteLimit
            )
        }
        return try readWithLineLimit(
            handle: handle,
            processHandle: processHandle,
            lineLimit: lineLimit,
            byteLimit: byteLimit
        )
    }

    private static func configureNonblockingRead(_ handle: FileHandle) {
        let descriptor = handle.fileDescriptor
        let flags = fcntl(descriptor, F_GETFL)
        if flags >= 0 {
            _ = fcntl(descriptor, F_SETFL, flags | O_NONBLOCK)
        }
    }

    private static func readWithByteLimit(
        handle: FileHandle,
        processHandle: ProcessHandle,
        byteLimit: Int?
    ) throws -> OutputRead {
        guard let byteLimit else {
            var collected = Data()
            while true {
                let chunk = try processHandle.read(from: handle)
                guard !chunk.isEmpty else {
                    return OutputRead(data: collected, truncated: false)
                }
                collected.append(chunk)
            }
        }

        var collected = Data()
        let chunkSize = 65536
        while true {
            let chunk = try processHandle.read(from: handle, maximumBytes: chunkSize)
            if chunk.isEmpty {
                return OutputRead(data: collected, truncated: false)
            }
            let remaining = byteLimit - collected.count
            guard chunk.count <= remaining else {
                if remaining > 0 {
                    collected.append(chunk.prefix(remaining))
                }
                processHandle.terminate()
                return OutputRead(data: collected, truncated: true)
            }
            collected.append(chunk)
        }
    }

    private static func readWithLineLimit(
        handle: FileHandle,
        processHandle: ProcessHandle,
        lineLimit: Int,
        byteLimit: Int?
    ) throws -> OutputRead {
        var collected = Data()
        var currentLineCount = 0
        let chunkSize = 65536

        while true {
            let chunk = try processHandle.read(from: handle, maximumBytes: chunkSize)
            if chunk.isEmpty {
                return OutputRead(data: collected, truncated: false)
            }

            if let byteLimit {
                let remaining = byteLimit - collected.count
                guard chunk.count <= remaining else {
                    if remaining > 0 {
                        collected.append(chunk.prefix(remaining))
                    }
                    processHandle.terminate()
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
                processHandle.terminate()
                return OutputRead(data: collected, truncated: true)
            }
        }
    }
}

private final class AsyncDataWriter: @unchecked Sendable {
    private let lock = NSLock()
    private let handle: FileHandle
    private let semaphore = DispatchSemaphore(value: 0)
    private var started = false
    private var cancelled = false
    private var finished = false

    init(handle: FileHandle) {
        self.handle = handle
    }

    func start(writing data: Data, on queue: DispatchQueue) {
        lock.lock()
        guard !started, !finished else {
            lock.unlock()
            return
        }
        started = true
        lock.unlock()
        queue.async { [self] in
            write(data)
            finish()
        }
    }

    func cancel() {
        lock.lock()
        cancelled = true
        let shouldFinish = !started && !finished
        lock.unlock()
        guard shouldFinish else { return }
        finish()
    }

    func waitUntilFinished() {
        semaphore.wait()
    }

    private func write(_ data: Data) {
        let descriptor = handle.fileDescriptor
        _ = fcntl(descriptor, F_SETNOSIGPIPE, 1)
        let currentFlags = fcntl(descriptor, F_GETFL)
        if currentFlags >= 0 {
            _ = fcntl(descriptor, F_SETFL, currentFlags | O_NONBLOCK)
        }
        data.withUnsafeBytes { bytes in
            guard let baseAddress = bytes.baseAddress else { return }
            var offset = 0
            while offset < bytes.count, !isCancelled {
                let writtenByteCount = Darwin.write(
                    descriptor,
                    baseAddress.advanced(by: offset),
                    bytes.count - offset
                )
                if writtenByteCount > 0 {
                    offset += writtenByteCount
                    continue
                }
                guard writtenByteCount < 0 else { return }
                if errno == EINTR {
                    continue
                }
                guard errno == EAGAIN || errno == EWOULDBLOCK else { return }
                var pollDescriptor = pollfd(
                    fd: descriptor,
                    events: Int16(POLLOUT | POLLERR | POLLHUP),
                    revents: 0
                )
                _ = Darwin.poll(&pollDescriptor, 1, 50)
            }
        }
    }

    private var isCancelled: Bool {
        lock.lock()
        defer { lock.unlock() }
        return cancelled
    }

    private func finish() {
        lock.lock()
        guard !finished else {
            lock.unlock()
            return
        }
        finished = true
        lock.unlock()
        try? handle.close()
        semaphore.signal()
    }
}

private struct OutputRead {
    let data: Data
    let truncated: Bool
}

private final class ProcessHandle: @unchecked Sendable {
    private let lock = NSLock()
    private var process: CancellableProcess?
    private var stdinWriter: AsyncDataWriter?
    private var cancelled = false

    var wasCancelled: Bool {
        lock.lock()
        defer { lock.unlock() }
        return cancelled
    }

    func attach(
        _ process: CancellableProcess,
        stdinWriter: AsyncDataWriter?
    ) -> Bool {
        lock.lock()
        self.process = process
        self.stdinWriter = stdinWriter
        let shouldTerminate = cancelled
        lock.unlock()
        guard shouldTerminate else { return true }
        process.terminateProcessGroup()
        stdinWriter?.cancel()
        return false
    }

    func detach() {
        lock.lock()
        process = nil
        stdinWriter = nil
        lock.unlock()
    }

    func terminate() {
        lock.lock()
        cancelled = true
        let process = process
        let stdinWriter = stdinWriter
        lock.unlock()
        process?.terminateProcessGroup()
        stdinWriter?.cancel()
    }

    func read(from handle: FileHandle, maximumBytes: Int = 65536) throws -> Data {
        let descriptor = handle.fileDescriptor
        var buffer = [UInt8](repeating: 0, count: maximumBytes)
        while !wasCancelled {
            let bytesRead = buffer.withUnsafeMutableBytes { bytes in
                Darwin.read(descriptor, bytes.baseAddress, bytes.count)
            }
            if bytesRead > 0 {
                return Data(buffer.prefix(Int(bytesRead)))
            }
            if bytesRead == 0 {
                return Data()
            }
            if errno == EINTR {
                continue
            }
            guard errno == EAGAIN || errno == EWOULDBLOCK else {
                throw NSError(domain: NSPOSIXErrorDomain, code: Int(errno))
            }
            var pollDescriptor = pollfd(
                fd: descriptor,
                events: Int16(POLLIN | POLLERR | POLLHUP),
                revents: 0
            )
            _ = Darwin.poll(&pollDescriptor, 1, 50)
        }
        return Data()
    }
}

private final class AsyncDataCollector: @unchecked Sendable {
    private let lock = NSLock()
    private var data = Data()
    private var truncated = false
    private let semaphore = DispatchSemaphore(value: 0)

    func start(
        reading handle: FileHandle,
        on queue: DispatchQueue,
        byteLimit: Int?,
        processHandle: ProcessHandle
    ) {
        queue.async { [self] in
            var collected = Data()
            var didTruncate = false
            while true {
                let chunk = (try? processHandle.read(from: handle)) ?? Data()
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
