import Darwin
import Foundation

final class ExecJobRegistry: @unchecked Sendable {
    private let maxJobsPerExtension: Int
    private let lock = NSLock()
    private var jobs: [String: ExecJob] = [:]
    private var activeCounts: [String: Int] = [:]

    init(maxJobsPerExtension: Int) {
        self.maxJobsPerExtension = maxJobsPerExtension
    }

    func insert(_ job: ExecJob) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        let active = activeCounts[job.extensionID, default: 0]
        guard active < maxJobsPerExtension, jobs[job.id] == nil else { return false }
        jobs[job.id] = job
        activeCounts[job.extensionID] = active + 1
        return true
    }

    func remove(id: String) {
        lock.lock()
        defer { lock.unlock() }
        guard let job = jobs.removeValue(forKey: id) else { return }
        let remaining = activeCounts[job.extensionID, default: 1] - 1
        activeCounts[job.extensionID] = remaining > 0 ? remaining : nil
    }

    func cancel(id: String, extensionID: String) -> Bool {
        lock.lock()
        let job = jobs[id]
        lock.unlock()
        guard let job, job.extensionID == extensionID else { return false }
        return job.cancel()
    }

    func cancelAll(extensionID: String) {
        lock.lock()
        let matching = jobs.values.filter { $0.extensionID == extensionID }
        lock.unlock()
        for job in matching {
            _ = job.cancel()
        }
    }
}

final class ExecJob: @unchecked Sendable {
    typealias Authorizer = @Sendable (ExecRequest, String) async throws -> WorkspaceContext

    let id: String
    let extensionID: String
    private let request: ExecRequest
    private let defaultCwd: String?
    private let authorizer: Authorizer?
    private let onCancellationClaimed: @Sendable () -> Void
    private let onRemove: @Sendable (String) -> Void
    private let lock = NSLock()
    private var completion: (@Sendable (Result<ExecResult, Error>) -> Void)?
    private var process: CancellableProcess?
    private var stdoutReader: OutputReader?
    private var stderrReader: OutputReader?
    private var stdoutBox: OutputBox?
    private var stderrBox: OutputBox?
    private var timedOut = false
    private var cancelled = false
    private var processExited = false
    private var finished = false
    private var authorizationTask: Task<Void, Never>?

    init(
        id: String,
        request: ExecRequest,
        extensionID: String,
        defaultCwd: String?,
        authorizer: Authorizer?,
        onCancellationClaimed: @escaping @Sendable () -> Void,
        completion: @escaping @Sendable (Result<ExecResult, Error>) -> Void,
        onRemove: @escaping @Sendable (String) -> Void
    ) {
        self.id = id
        self.request = request
        self.extensionID = extensionID
        self.defaultCwd = defaultCwd
        self.authorizer = authorizer
        self.onCancellationClaimed = onCancellationClaimed
        self.completion = completion
        self.onRemove = onRemove
    }

    func authorizeAndRun() {
        guard let authorizer else {
            finish(.failure(ExecError.launchFailed("missing authorization handler")))
            return
        }
        let task = Task { [weak self] in
            guard let self else { return }
            do {
                let context = try await authorizer(request, extensionID)
                try Task.checkCancellation()
                run(context: context)
            } catch {
                finish(.failure(Task.isCancelled ? ExecError.cancelled : error))
            }
        }
        lock.lock()
        if finished || cancelled {
            lock.unlock()
            task.cancel()
            return
        }
        authorizationTask = task
        lock.unlock()
    }

    func run(context: WorkspaceContext) {
        lock.lock()
        let shouldSkip = finished || cancelled
        lock.unlock()
        guard !shouldSkip else {
            finish(.failure(ExecError.cancelled))
            return
        }

        let process = Process()
        do {
            try ExtensionCommandExecutor.configureLaunch(
                process,
                request: request,
                extensionID: extensionID,
                defaultCwd: defaultCwd,
                context: context
            )
        } catch {
            finish(.failure(error))
            return
        }

        let stdoutPipe = Pipe()
        let stderrPipe = Pipe()
        let stdinPipe = Pipe()
        process.standardOutput = stdoutPipe
        process.standardError = stderrPipe
        process.standardInput = stdinPipe

        let stdoutBox = OutputBox()
        let stderrBox = OutputBox()
        let stdoutReader = OutputReader(pipe: stdoutPipe, box: stdoutBox)
        let stderrReader = OutputReader(pipe: stderrPipe, box: stderrBox)
        stdoutReader.start()
        stderrReader.start()

        lock.lock()
        if finished || cancelled {
            lock.unlock()
            finish(.failure(ExecError.cancelled))
            return
        }
        self.stdoutReader = stdoutReader
        self.stderrReader = stderrReader
        self.stdoutBox = stdoutBox
        self.stderrBox = stderrBox
        do {
            self.process = try CancellableProcess.launch(
                configuredProcess: process,
                stdinPipe: stdinPipe,
                stdoutPipe: stdoutPipe,
                stderrPipe: stderrPipe
            ) { [weak self] in
                self?.processDidTerminate()
            }
        } catch {
            self.process = nil
            self.stdoutReader = nil
            self.stderrReader = nil
            self.stdoutBox = nil
            self.stderrBox = nil
            lock.unlock()
            stdoutReader.finish()
            stderrReader.finish()
            finish(.failure(ExecError.launchFailed(error.localizedDescription)))
            return
        }
        lock.unlock()

        let timeoutMs = request.timeoutMs ?? ExtensionCommandExecutor.defaultTimeoutMs
        if timeoutMs > 0 {
            scheduleTimeout(after: timeoutMs)
        }

        let stdinWrite = StdinWrite(pipe: stdinPipe, text: request.stdin)
        DispatchQueue.global(qos: .utility).async {
            stdinWrite.perform()
        }
    }

    func fail(_ error: ExecError) {
        finish(.failure(error))
    }

    func cancel() -> Bool {
        lock.lock()
        guard !finished, !cancelled, !timedOut, !processExited else {
            lock.unlock()
            return false
        }
        let runningProcess = process
        let authorizationTask = authorizationTask
        if let runningProcess {
            guard runningProcess.terminate() else {
                lock.unlock()
                return false
            }
        }
        cancelled = true
        self.authorizationTask = nil
        lock.unlock()

        onCancellationClaimed()
        authorizationTask?.cancel()
        if runningProcess != nil {
            return true
        }
        finish(.failure(ExecError.cancelled))
        return true
    }

    private func scheduleTimeout(after milliseconds: Int) {
        DispatchQueue.global(qos: .userInitiated).asyncAfter(deadline: .now() + .milliseconds(milliseconds)) { [weak self] in
            self?.timeout()
        }
    }

    private func timeout() {
        lock.lock()
        guard !finished, !cancelled, !timedOut, !processExited,
              let runningProcess = process,
              runningProcess.terminate()
        else {
            lock.unlock()
            return
        }
        timedOut = true
        lock.unlock()
    }

    private func processDidTerminate() {
        lock.lock()
        guard !processExited else {
            lock.unlock()
            return
        }
        processExited = true
        let wasCancelled = cancelled
        let didTimeOut = timedOut
        let status = process?.terminationStatus ?? -1
        let outputReaders = (stdoutReader, stderrReader)
        let stdout = stdoutBox
        let stderr = stderrBox
        lock.unlock()

        outputReaders.0?.finish()
        outputReaders.1?.finish()

        if wasCancelled {
            finish(.failure(ExecError.cancelled))
            return
        }

        finish(.success(ExecResult(
            stdout: stdout?.string() ?? "",
            stderr: stderr?.string() ?? "",
            exitCode: status,
            timedOut: didTimeOut,
            truncated: (stdout?.overflow ?? false) || (stderr?.overflow ?? false)
        )))
    }

    private func finish(_ result: Result<ExecResult, Error>) {
        let callback: (@Sendable (Result<ExecResult, Error>) -> Void)?
        let deliveredResult: Result<ExecResult, Error>
        lock.lock()
        guard !finished else {
            lock.unlock()
            return
        }
        finished = true
        deliveredResult = cancelled ? .failure(ExecError.cancelled) : result
        callback = completion
        completion = nil
        authorizationTask = nil
        process = nil
        stdoutReader = nil
        stderrReader = nil
        stdoutBox = nil
        stderrBox = nil
        lock.unlock()

        onRemove(id)
        callback?(deliveredResult)
    }
}

private struct StdinWrite: @unchecked Sendable {
    let pipe: Pipe
    let text: String?

    func perform() {
        let handle = pipe.fileHandleForWriting
        defer {
            try? handle.close()
        }
        guard let text, !text.isEmpty else { return }
        _ = fcntl(handle.fileDescriptor, F_SETNOSIGPIPE, 1)
        try? handle.write(contentsOf: Data(text.utf8))
    }
}
