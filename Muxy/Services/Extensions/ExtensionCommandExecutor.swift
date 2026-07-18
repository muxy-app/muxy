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
        isCancelled: @escaping @Sendable () -> Bool = { false },
        onCancellationClaimed: @escaping @Sendable () -> Void = {},
        authorize: @escaping ExecJob.Authorizer = { request, extensionID in
            try await ExtensionCommandExecutor.authorizeCancelableExec(
                request: request,
                extensionID: extensionID
            )
        },
        completion: @escaping @Sendable (Result<ExecResult, Error>) -> Void
    ) -> String {
        let job = ExecJob(
            id: jobID,
            request: request,
            extensionID: extensionID,
            defaultCwd: defaultCwd,
            authorizer: authorize,
            onCancellationClaimed: onCancellationClaimed,
            completion: completion,
            onRemove: { id in jobs.remove(id: id) }
        )
        jobs.insert(job)
        guard !isCancelled() else {
            _ = jobs.cancel(id: job.id)
            return job.id
        }
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
            authorizer: nil,
            onCancellationClaimed: {},
            completion: completion,
            onRemove: { id in jobs.remove(id: id) }
        )
        jobs.insert(job)
        job.run(context: context)
        return job.id
    }

    static func cancelExec(jobID: String) -> Bool {
        jobs.cancel(id: jobID)
    }

    static func cancelExec(jobID: String, extensionID: String) -> Bool {
        jobs.cancel(id: jobID, extensionID: extensionID)
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
        let stdoutReader = OutputReader(pipe: stdoutPipe, box: stdoutBox)
        let stderrReader = OutputReader(pipe: stderrPipe, box: stderrBox)
        let timeoutFlag = TimeoutFlag()

        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            stdoutReader.start()
            stderrReader.start()

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

        stdoutReader.finish()
        stderrReader.finish()

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

    static func authorizeCancelableExec(request: ExecRequest, extensionID: String) async throws -> WorkspaceContext {
        try await authorizeExec(request: request, extensionID: extensionID)
    }

    static func configureLaunch(
        _ process: Process,
        request: ExecRequest,
        extensionID: String,
        defaultCwd: String?,
        context: WorkspaceContext
    ) throws {
        let cwdValue = request.cwd ?? defaultCwd
        guard cwdValue?.contains("\0") != true else {
            throw ExecError.invalidArguments("cwd cannot contain null bytes")
        }
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

    static func writeStdin(_ text: String?, into pipe: Pipe) {
        let handle = pipe.fileHandleForWriting
        defer {
            try? handle.close()
        }
        guard let text, !text.isEmpty else { return }
        _ = fcntl(handle.fileDescriptor, F_SETNOSIGPIPE, 1)
        try? handle.write(contentsOf: Data(text.utf8))
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
