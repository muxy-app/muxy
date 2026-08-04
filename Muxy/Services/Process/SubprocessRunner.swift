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

    init(
        executablePath: String,
        arguments: [String] = [],
        workingDirectory: String? = nil,
        environment: [String: String] = ProcessInfo.processInfo.environment,
        standardInput: Data? = nil,
        timeout: TimeInterval? = nil,
        outputByteLimit: Int = 64 * 1024
    ) {
        self.executablePath = executablePath
        self.arguments = arguments
        self.workingDirectory = workingDirectory
        self.environment = environment
        self.standardInput = standardInput
        self.timeout = timeout
        self.outputByteLimit = outputByteLimit
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
    private let request: SubprocessRequest
    private let lock = NSLock()
    private var continuation: CheckedContinuation<SubprocessResult, Error>?
    private var process: CancellableProcess?
    private var stdoutReader: OutputReader?
    private var stderrReader: OutputReader?
    private var stdout: OutputBox?
    private var stderr: OutputBox?
    private var timedOut = false
    private var cancelled = false
    private var finished = false

    init(request: SubprocessRequest) {
        self.request = request
    }

    func run() async throws -> SubprocessResult {
        try await withCheckedThrowingContinuation { continuation in
            lock.lock()
            self.continuation = continuation
            let wasCancelled = cancelled
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
        cancelled = true
        let process = process
        lock.unlock()
        guard let process else { finish(.failure(SubprocessRunnerError.cancelled))
            return
        }
        _ = process.terminate()
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
        let stdoutReader = OutputReader(pipe: stdoutPipe, box: stdout), stderrReader = OutputReader(pipe: stderrPipe, box: stderr)
        stdoutReader.start()
        stderrReader.start()
        lock.lock()
        guard !cancelled else { lock.unlock()
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
            stderrPipe: stderrPipe
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
            DispatchQueue.global(qos: .utility).asyncAfter(deadline: .now() + timeout) { [weak self] in self?.timeout() }
        }
    }

    private func timeout() {
        lock.lock()
        guard !finished, !cancelled, let process else { lock.unlock()
            return
        }
        timedOut = true
        lock.unlock()
        _ = process.terminate()
    }

    private func didTerminate() {
        lock.lock()
        let wasCancelled = cancelled
        let didTimeOut = timedOut
        let status = process?.terminationStatus ?? -1
        let readers = (stdoutReader, stderrReader)
        let output = (stdout, stderr)
        lock.unlock()
        readers.0?.finish()
        readers.1?.finish()
        if wasCancelled {
            finish(.failure(SubprocessRunnerError.cancelled))
            return
        }
        if didTimeOut, let timeout = request.timeout {
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
