import Darwin
import Foundation

struct ExecRequest {
    let argv: [String]?
    let shell: String?
    let cwd: String?
    let env: [String: String]?
    let stdin: String?
    let timeoutMs: Int?
}

struct ExecResult {
    let stdout: String
    let stderr: String
    let exitCode: Int32
    let timedOut: Bool
    let truncated: Bool
}

enum ExecError: Error, LocalizedError {
    case invalidArguments(String)
    case launchFailed(String)
    case cancelled

    var errorDescription: String? {
        switch self {
        case let .invalidArguments(detail): "exec: \(detail)"
        case let .launchFailed(detail): "exec failed to launch: \(detail)"
        case .cancelled: "exec cancelled"
        }
    }
}

enum ExtensionCommandExecutor {
    static let defaultTimeoutMs = 30000
    static let maxOutputBytes = 10 * 1024 * 1024
    private static let jobs = ExecJobRegistry()

    @MainActor
    static func exec(
        request: ExecRequest,
        extensionID: String,
        defaultCwd: String?
    ) async throws -> ExecResult {
        let context = try await authorizeExec(request: request, extensionID: extensionID)
        return try await runUnchecked(
            request: request,
            extensionID: extensionID,
            defaultCwd: defaultCwd,
            context: context
        )
    }

    static func startCancelableExec(
        jobID: String = UUID().uuidString,
        request: ExecRequest,
        extensionID: String,
        defaultCwd: String?,
        completion: @escaping @Sendable (Result<ExecResult, Error>) -> Void
    ) -> String {
        let job = ExecJob(
            id: jobID,
            request: request,
            extensionID: extensionID,
            defaultCwd: defaultCwd,
            completion: completion
        ) { id in
            jobs.remove(id: id)
        }
        jobs.insert(job)
        job.authorizeAndRun()
        return job.id
    }

    static func startCancelableUnchecked(
        jobID: String = UUID().uuidString,
        request: ExecRequest,
        extensionID: String,
        defaultCwd: String?,
        context: WorkspaceContext = .local,
        completion: @escaping @Sendable (Result<ExecResult, Error>) -> Void
    ) -> String {
        let job = ExecJob(
            id: jobID,
            request: request,
            extensionID: extensionID,
            defaultCwd: defaultCwd,
            completion: completion
        ) { id in
            jobs.remove(id: id)
        }
        jobs.insert(job)
        job.run(context: context)
        return job.id
    }

    static func cancelExec(jobID: String) -> Bool {
        jobs.cancel(id: jobID)
    }

    static func cancelExec(extensionID: String) {
        jobs.cancelAll(extensionID: extensionID)
    }

    static func runUnchecked(
        request: ExecRequest,
        extensionID: String,
        defaultCwd: String?,
        context: WorkspaceContext = .local
    ) async throws -> ExecResult {
        let process = Process()
        try configureLaunch(
            process,
            request: request,
            extensionID: extensionID,
            defaultCwd: defaultCwd,
            context: context
        )

        let stdoutPipe = Pipe()
        let stderrPipe = Pipe()
        let stdinPipe = Pipe()
        process.standardOutput = stdoutPipe
        process.standardError = stderrPipe
        process.standardInput = stdinPipe

        let stdoutBox = OutputBox()
        let stderrBox = OutputBox()
        let timeoutFlag = TimeoutFlag()

        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            attachReader(pipe: stdoutPipe, box: stdoutBox)
            attachReader(pipe: stderrPipe, box: stderrBox)

            let resumeBox = ResumeBox(continuation: continuation)
            process.terminationHandler = { _ in
                resumeBox.resume()
            }

            do {
                try process.run()
            } catch {
                resumeBox.resume(throwing: ExecError.launchFailed(error.localizedDescription))
                return
            }

            writeStdin(request.stdin, into: stdinPipe)

            let timeoutMs = request.timeoutMs ?? defaultTimeoutMs
            if timeoutMs > 0 {
                scheduleTimeout(process: process, after: timeoutMs, flag: timeoutFlag)
            }
        } as Void

        return ExecResult(
            stdout: stdoutBox.string(),
            stderr: stderrBox.string(),
            exitCode: process.terminationStatus,
            timedOut: timeoutFlag.fired,
            truncated: stdoutBox.overflow || stderrBox.overflow
        )
    }

    @MainActor
    private static func authorizeExec(request: ExecRequest, extensionID: String) async throws -> WorkspaceContext {
        guard ExtensionStore.shared.extensionHasPermission(id: extensionID, permission: .commandsExec) else {
            throw ExecError.invalidArguments("permission denied (\(ExtensionPermission.commandsExec.rawValue))")
        }
        let consentRequest = ExtensionConsentRequestBuilder.make(
            extensionID: extensionID,
            verb: .exec,
            payload: .exec(argv: request.argv, shell: request.shell),
            source: "exec"
        )
        let decision = await ExtensionConsentService.shared.gate(consentRequest)
        guard decision == .allow else {
            throw ExecError.invalidArguments("user denied consent for exec")
        }
        return ActiveWorkspaceContext.shared.current
    }

    fileprivate static func authorizeCancelableExec(request: ExecRequest, extensionID: String) async throws -> WorkspaceContext {
        try await authorizeExec(request: request, extensionID: extensionID)
    }

    fileprivate static func configureLaunch(
        _ process: Process,
        request: ExecRequest,
        extensionID: String,
        defaultCwd: String?,
        context: WorkspaceContext
    ) throws {
        let cwdValue = request.cwd ?? defaultCwd
        guard !context.isRemote else {
            try configureRemoteLaunch(process, request: request, cwdValue: cwdValue, context: context)
            return
        }

        if let shell = request.shell {
            process.executableURL = URL(fileURLWithPath: "/bin/sh")
            process.arguments = ["-c", shell]
        } else if let argv = request.argv, let head = argv.first, !head.isEmpty {
            process.executableURL = try URL(fileURLWithPath: resolveExecutable(head))
            process.arguments = Array(argv.dropFirst())
        } else {
            throw ExecError.invalidArguments("either argv (non-empty) or shell is required")
        }

        if let cwdValue, !cwdValue.isEmpty {
            let expanded = NSString(string: cwdValue).expandingTildeInPath
            process.currentDirectoryURL = URL(fileURLWithPath: expanded)
        }

        var environment = ProcessInfo.processInfo.environment
        environment["PATH"] = LoginShellPath.current
        if let extra = request.env {
            for (key, value) in extra where isSafeEnvKey(key) {
                environment[key] = value
            }
        }
        environment["MUXY_EXTENSION_ID"] = extensionID
        process.environment = environment
    }

    private static func configureRemoteLaunch(
        _ process: Process,
        request: ExecRequest,
        cwdValue: String?,
        context: WorkspaceContext
    ) throws {
        let workingDirectory = (cwdValue?.isEmpty == false) ? cwdValue : nil
        let remoteEnv = request.env?.filter { isSafeEnvKey($0.key) }
        let resolved: ResolvedLaunch
        if let shell = request.shell {
            resolved = CommandTransform.resolveShell(
                shellCommand: shell,
                workingDirectory: workingDirectory,
                environment: remoteEnv,
                in: context
            )
        } else if let argv = request.argv, let head = argv.first, !head.isEmpty {
            resolved = CommandTransform.resolve(
                executable: head,
                arguments: Array(argv.dropFirst()),
                workingDirectory: workingDirectory,
                environment: remoteEnv,
                in: context
            )
        } else {
            throw ExecError.invalidArguments("either argv (non-empty) or shell is required")
        }
        process.executableURL = URL(fileURLWithPath: resolved.executable)
        process.arguments = resolved.arguments
    }

    private static func isSafeEnvKey(_ key: String) -> Bool {
        guard !key.isEmpty,
              !key.contains("="),
              !key.contains("\0"),
              !key.hasPrefix("DYLD_"),
              key != "MUXY_EXTENSION_ID"
        else { return false }
        return true
    }

    private static func resolveExecutable(_ command: String) throws -> String {
        if command.contains("/") {
            return command
        }
        let pathEnv = LoginShellPath.current
        for directory in pathEnv.split(separator: ":") {
            let candidate = URL(fileURLWithPath: String(directory))
                .appendingPathComponent(command)
            if FileManager.default.isExecutableFile(atPath: candidate.path) {
                return candidate.path
            }
        }
        throw ExecError.launchFailed("command not found: \(command)")
    }

    fileprivate static func attachReader(pipe: Pipe, box: OutputBox) {
        pipe.fileHandleForReading.readabilityHandler = { handle in
            let data = handle.availableData
            if data.isEmpty {
                handle.readabilityHandler = nil
                return
            }
            box.append(data)
        }
    }

    fileprivate static func writeStdin(_ text: String?, into pipe: Pipe) {
        defer {
            try? pipe.fileHandleForWriting.close()
        }
        guard let text, !text.isEmpty else { return }
        try? pipe.fileHandleForWriting.write(contentsOf: Data(text.utf8))
    }

    fileprivate static func scheduleTimeout(process: Process, after milliseconds: Int, flag: TimeoutFlag) {
        DispatchQueue.global(qos: .userInitiated).asyncAfter(deadline: .now() + .milliseconds(milliseconds)) {
            guard process.isRunning else { return }
            flag.fired = true
            process.terminate()
            DispatchQueue.global(qos: .userInitiated).asyncAfter(deadline: .now() + .seconds(2)) {
                guard process.isRunning else { return }
                kill(process.processIdentifier, SIGKILL)
            }
        }
    }
}

private final class ExecJobRegistry: @unchecked Sendable {
    private let lock = NSLock()
    private var jobs: [String: ExecJob] = [:]

    func insert(_ job: ExecJob) {
        lock.lock()
        jobs[job.id] = job
        lock.unlock()
    }

    func remove(id: String) {
        lock.lock()
        jobs[id] = nil
        lock.unlock()
    }

    func cancel(id: String) -> Bool {
        lock.lock()
        let job = jobs[id]
        lock.unlock()
        return job?.cancel() ?? false
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

private final class ExecJob: @unchecked Sendable {
    let id: String
    let extensionID: String
    private let request: ExecRequest
    private let defaultCwd: String?
    private let onRemove: @Sendable (String) -> Void
    private let lock = NSLock()
    private var completion: (@Sendable (Result<ExecResult, Error>) -> Void)?
    private var process: Process?
    private var stdoutPipe: Pipe?
    private var stderrPipe: Pipe?
    private var stdoutBox: OutputBox?
    private var stderrBox: OutputBox?
    private var timedOut = false
    private var cancelled = false
    private var finished = false

    init(
        id: String,
        request: ExecRequest,
        extensionID: String,
        defaultCwd: String?,
        completion: @escaping @Sendable (Result<ExecResult, Error>) -> Void,
        onRemove: @escaping @Sendable (String) -> Void
    ) {
        self.id = id
        self.request = request
        self.extensionID = extensionID
        self.defaultCwd = defaultCwd
        self.completion = completion
        self.onRemove = onRemove
    }

    func authorizeAndRun() {
        Task {
            do {
                let context = try await ExtensionCommandExecutor.authorizeCancelableExec(
                    request: request,
                    extensionID: extensionID
                )
                run(context: context)
            } catch {
                finish(.failure(error))
            }
        }
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
        ExtensionCommandExecutor.attachReader(pipe: stdoutPipe, box: stdoutBox)
        ExtensionCommandExecutor.attachReader(pipe: stderrPipe, box: stderrBox)

        process.terminationHandler = { [weak self] _ in
            self?.processDidTerminate()
        }

        lock.lock()
        if finished || cancelled {
            lock.unlock()
            finish(.failure(ExecError.cancelled))
            return
        }
        self.process = process
        self.stdoutPipe = stdoutPipe
        self.stderrPipe = stderrPipe
        self.stdoutBox = stdoutBox
        self.stderrBox = stderrBox
        do {
            try process.run()
        } catch {
            self.process = nil
            self.stdoutPipe = nil
            self.stderrPipe = nil
            self.stdoutBox = nil
            self.stderrBox = nil
            lock.unlock()
            finish(.failure(ExecError.launchFailed(error.localizedDescription)))
            return
        }
        lock.unlock()

        ExtensionCommandExecutor.writeStdin(request.stdin, into: stdinPipe)

        let timeoutMs = request.timeoutMs ?? ExtensionCommandExecutor.defaultTimeoutMs
        if timeoutMs > 0 {
            scheduleTimeout(after: timeoutMs)
        }
    }

    func cancel() -> Bool {
        lock.lock()
        guard !finished, !cancelled else {
            lock.unlock()
            return false
        }
        cancelled = true
        let runningProcess = process
        lock.unlock()

        if let runningProcess, runningProcess.isRunning {
            runningProcess.terminate()
            DispatchQueue.global(qos: .userInitiated).asyncAfter(deadline: .now() + .seconds(2)) {
                guard runningProcess.isRunning else { return }
                kill(runningProcess.processIdentifier, SIGKILL)
            }
        } else {
            finish(.failure(ExecError.cancelled))
        }
        return true
    }

    private func scheduleTimeout(after milliseconds: Int) {
        DispatchQueue.global(qos: .userInitiated).asyncAfter(deadline: .now() + .milliseconds(milliseconds)) { [weak self] in
            self?.timeout()
        }
    }

    private func timeout() {
        lock.lock()
        guard !finished, !cancelled, let runningProcess = process, runningProcess.isRunning else {
            lock.unlock()
            return
        }
        timedOut = true
        lock.unlock()

        runningProcess.terminate()
        DispatchQueue.global(qos: .userInitiated).asyncAfter(deadline: .now() + .seconds(2)) {
            guard runningProcess.isRunning else { return }
            kill(runningProcess.processIdentifier, SIGKILL)
        }
    }

    private func processDidTerminate() {
        lock.lock()
        let wasCancelled = cancelled
        let didTimeOut = timedOut
        let status = process?.terminationStatus ?? -1
        let outputPipes = (stdoutPipe, stderrPipe)
        let stdout = stdoutBox
        let stderr = stderrBox
        lock.unlock()

        drain(pipe: outputPipes.0, into: stdout)
        drain(pipe: outputPipes.1, into: stderr)

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
        lock.lock()
        guard !finished else {
            lock.unlock()
            return
        }
        finished = true
        callback = completion
        completion = nil
        process = nil
        stdoutPipe = nil
        stderrPipe = nil
        stdoutBox = nil
        stderrBox = nil
        lock.unlock()

        onRemove(id)
        callback?(result)
    }

    private func drain(pipe: Pipe?, into box: OutputBox?) {
        guard let pipe, let box else { return }
        let handle = pipe.fileHandleForReading
        handle.readabilityHandler = nil
        let data = handle.readDataToEndOfFile()
        if !data.isEmpty {
            box.append(data)
        }
    }
}

private final class TimeoutFlag: @unchecked Sendable {
    private let lock = NSLock()
    private var didFire = false

    var fired: Bool {
        get { lock.lock()
            defer { lock.unlock() }
            return didFire
        }
        set { lock.lock()
            defer { lock.unlock() }
            didFire = newValue
        }
    }
}

private final class OutputBox: @unchecked Sendable {
    private let lock = NSLock()
    private var data = Data()
    private(set) var overflow = false

    func append(_ chunk: Data) {
        lock.lock()
        defer { lock.unlock() }
        if overflow { return }
        let remaining = ExtensionCommandExecutor.maxOutputBytes - data.count
        if chunk.count <= remaining {
            data.append(chunk)
            return
        }
        if remaining > 0 {
            data.append(chunk.prefix(remaining))
        }
        overflow = true
    }

    func string() -> String {
        lock.lock()
        defer { lock.unlock() }
        return String(data: data, encoding: .utf8) ?? ""
    }
}

private final class ResumeBox: @unchecked Sendable {
    private let lock = NSLock()
    private var continuation: CheckedContinuation<Void, Error>?

    init(continuation: CheckedContinuation<Void, Error>) {
        self.continuation = continuation
    }

    func resume() {
        lock.lock()
        defer { lock.unlock() }
        continuation?.resume()
        continuation = nil
    }

    func resume(throwing error: Error) {
        lock.lock()
        defer { lock.unlock() }
        continuation?.resume(throwing: error)
        continuation = nil
    }
}
