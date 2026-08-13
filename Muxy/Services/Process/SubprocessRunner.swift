import Darwin
import Foundation

struct SubprocessRequest: Sendable {
    let executablePath: String
    let arguments: [String]
    let workingDirectory: String?
    let environment: [String: String]
    let standardInput: Data?
    let timeout: TimeInterval?
    let outputByteLimit: Int
    let onStandardOutput: (@Sendable (Data) -> Void)?
    let onStandardError: (@Sendable (Data) -> Void)?

    init(
        executablePath: String,
        arguments: [String] = [],
        workingDirectory: String? = nil,
        environment: [String: String] = ProcessInfo.processInfo.environment,
        standardInput: Data? = nil,
        timeout: TimeInterval? = nil,
        outputByteLimit: Int = 64 * 1024,
        onStandardOutput: (@Sendable (Data) -> Void)? = nil,
        onStandardError: (@Sendable (Data) -> Void)? = nil
    ) {
        self.executablePath = executablePath
        self.arguments = arguments
        self.workingDirectory = workingDirectory
        self.environment = environment
        self.standardInput = standardInput
        self.timeout = timeout
        self.outputByteLimit = outputByteLimit
        self.onStandardOutput = onStandardOutput
        self.onStandardError = onStandardError
    }
}

struct SubprocessResult: Sendable {
    let status: Int32
    let stdoutData: Data
    let stderrData: Data
    let truncated: Bool
    var stdout: String { String(data: stdoutData, encoding: .utf8) ?? "" }
    var stderr: String { String(data: stderrData, encoding: .utf8) ?? "" }
}

enum SubprocessRunnerError: LocalizedError, Equatable {
    case launchFailed(String)
    case timedOut(TimeInterval)
    case cancelled

    var errorDescription: String? {
        switch self {
        case let .launchFailed(detail): "Process failed to launch: \(detail)"
        case let .timedOut(value): "Process timed out after \(Int(value))s"
        case .cancelled: "Process cancelled"
        }
    }
}

enum SubprocessRunner {
    static func run(_ request: SubprocessRequest) async throws -> SubprocessResult {
        let job = SubprocessJob(request: request)
        return try await withTaskCancellationHandler { try await job.run() } onCancel: { job.cancel() }
    }
}

private final class SubprocessJob: @unchecked Sendable {
    private static let timeoutQueue = DispatchQueue(
        label: "app.muxy.subprocess-timeout",
        qos: .userInitiated
    )
    private static let monitoringQueue = DispatchQueue(
        label: "app.muxy.subprocess-monitor",
        qos: .userInitiated,
        attributes: .concurrent
    )

    private enum TerminationReason {
        case timedOut(TimeInterval)
        case cancelled
    }

    private let request: SubprocessRequest
    private let lock = NSLock()
    private var continuation: CheckedContinuation<SubprocessResult, Error>?
    private var process: CancellableProcess?
    private var stdoutReader: OutputReader?
    private var stderrReader: OutputReader?
    private var stdout: OutputBox?
    private var stderr: OutputBox?
    private var terminationReason: TerminationReason?
    private var finished = false
    private var processExited = false
    private var stdoutFinished = false
    private var stderrFinished = false

    init(request: SubprocessRequest) {
        self.request = request
    }

    func run() async throws -> SubprocessResult {
        try await withCheckedThrowingContinuation { continuation in
            lock.lock()
            self.continuation = continuation
            let wasCancelled = terminationReason != nil
            lock.unlock()
            guard !wasCancelled else {
                finish(.failure(SubprocessRunnerError.cancelled))
                return
            }
            launch()
        }
    }

    func cancel() {
        lock.lock()
        guard !finished else { lock.unlock()
            return
        }
        if terminationReason == nil {
            terminationReason = .cancelled
        }
        let process = process
        lock.unlock()
        guard let process else { finish(.failure(SubprocessRunnerError.cancelled))
            return
        }
        process.terminateProcessGroup()
    }

    private func launch() {
        let configured = Process()
        configured.executableURL = URL(fileURLWithPath: request.executablePath)
        configured.arguments = request.arguments
        configured.environment = request.environment
        if let workingDirectory = request.workingDirectory {
            configured.currentDirectoryURL = URL(fileURLWithPath: workingDirectory)
        }
        let stdin = Pipe(), stdoutPipe = Pipe(), stderrPipe = Pipe()
        configured.standardInput = stdin
        configured.standardOutput = stdoutPipe
        configured.standardError = stderrPipe
        let stdout = OutputBox(maximumBytes: request.outputByteLimit), stderr = OutputBox(maximumBytes: request.outputByteLimit)
        let stdoutReader = OutputReader(
            pipe: stdoutPipe,
            box: stdout,
            onData: request.onStandardOutput,
            onFinish: { [weak self] in self?.outputFinished(stdout: true) }
        )
        let stderrReader = OutputReader(
            pipe: stderrPipe,
            box: stderr,
            onData: request.onStandardError,
            onFinish: { [weak self] in self?.outputFinished(stdout: false) }
        )
        stdoutReader.start()
        stderrReader.start()
        lock.lock()
        guard terminationReason == nil else { lock.unlock()
            stdoutReader.finish()
            stderrReader.finish()
            finish(.failure(SubprocessRunnerError.cancelled))
            return
        }
        self.stdout = stdout
        self.stderr = stderr
        self.stdoutReader = stdoutReader
        self.stderrReader = stderrReader
        do { self.process = try CancellableProcess.launch(
            configuredProcess: configured,
            stdinPipe: stdin,
            stdoutPipe: stdoutPipe,
            stderrPipe: stderrPipe,
            monitoringQueue: Self.monitoringQueue,
            deferReaping: true
        ) { [weak self] in self?.didTerminate() } } catch { lock.unlock()
            stdoutReader.finish()
            stderrReader.finish()
            finish(.failure(SubprocessRunnerError.launchFailed(error.localizedDescription)))
            return
        }
        lock.unlock()
        DispatchQueue.global(qos: .utility).async { [input = request.standardInput] in let handle = stdin.fileHandleForWriting
            defer { try? handle.close() }
            if let input {
                _ = fcntl(handle.fileDescriptor, F_SETNOSIGPIPE, 1)
                try? handle.write(contentsOf: input)
            }
        }
        if let timeout = request
            .timeout
        {
            Self.timeoutQueue.asyncAfter(deadline: .now() + timeout) { [weak self] in self?.timeout() }
        }
    }

    private func timeout() {
        lock.lock()
        guard !finished, terminationReason == nil, let process else { lock.unlock()
            return
        }
        terminationReason = .timedOut(request.timeout ?? 0)
        lock.unlock()
        process.terminateProcessGroup()
    }

    private func didTerminate() {
        lock.lock()
        processExited = true
        let shouldForceFinishOutput = terminationReason != nil
        let readers = (stdoutReader, stderrReader)
        lock.unlock()
        if shouldForceFinishOutput {
            readers.0?.finish()
            readers.1?.finish()
        }
        finishIfReady()
    }

    private func outputFinished(stdout: Bool) {
        lock.lock()
        if stdout {
            stdoutFinished = true
        } else {
            stderrFinished = true
        }
        let shouldRelease = stdoutFinished && stderrFinished
        let process = process
        lock.unlock()
        if shouldRelease {
            process?.releaseReaping()
        }
        finishIfReady()
    }

    private func finishIfReady() {
        lock.lock()
        guard processExited, stdoutFinished, stderrFinished else {
            lock.unlock()
            return
        }
        let terminationReason = terminationReason
        let status = process?.terminationStatus ?? -1
        let output = (stdout, stderr)
        lock.unlock()
        if case .cancelled = terminationReason {
            finish(.failure(SubprocessRunnerError.cancelled))
            return
        }
        if case let .timedOut(timeout) = terminationReason {
            finish(.failure(SubprocessRunnerError.timedOut(timeout)))
            return
        }
        finish(.success(SubprocessResult(
            status: status,
            stdoutData: output.0?.value() ?? Data(),
            stderrData: output.1?.value() ?? Data(),
            truncated: (output.0?.overflow ?? false) || (output.1?.overflow ?? false)
        )))
    }

    private func finish(_ result: Result<SubprocessResult, Error>) {
        lock.lock()
        guard !finished, let continuation else { lock.unlock()
            return
        }
        finished = true
        self.continuation = nil
        process = nil
        stdoutReader = nil
        stderrReader = nil
        stdout = nil
        stderr = nil
        lock.unlock()
        continuation.resume(with: result)
    }
}
