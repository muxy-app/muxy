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

    var errorDescription: String? {
        switch self {
        case let .invalidArguments(detail): "exec: \(detail)"
        case let .launchFailed(detail): "exec failed to launch: \(detail)"
        }
    }
}

enum ExtensionCommandExecutor {
    static let defaultTimeoutMs = 30000
    static let maxOutputBytes = 10 * 1024 * 1024

    @MainActor
    static func exec(
        request: ExecRequest,
        extensionID: String,
        defaultCwd: String?
    ) async throws -> ExecResult {
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
        let context = ActiveWorkspaceContext.shared.current
        return try await runUnchecked(
            request: request,
            extensionID: extensionID,
            defaultCwd: defaultCwd,
            context: context
        )
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

    private static func configureLaunch(
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

    private static func attachReader(pipe: Pipe, box: OutputBox) {
        pipe.fileHandleForReading.readabilityHandler = { handle in
            let data = handle.availableData
            if data.isEmpty {
                handle.readabilityHandler = nil
                return
            }
            box.append(data)
        }
    }

    private static func writeStdin(_ text: String?, into pipe: Pipe) {
        defer {
            try? pipe.fileHandleForWriting.close()
        }
        guard let text, !text.isEmpty else { return }
        try? pipe.fileHandleForWriting.write(contentsOf: Data(text.utf8))
    }

    private static func scheduleTimeout(process: Process, after milliseconds: Int, flag: TimeoutFlag) {
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
