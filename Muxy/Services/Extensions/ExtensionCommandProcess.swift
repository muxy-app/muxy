import Darwin
import Foundation

final class CancellableProcess: @unchecked Sendable {
    private static let controlQueue = DispatchQueue(
        label: "app.muxy.cancellable-process-control",
        qos: .userInitiated,
        attributes: .concurrent
    )

    private enum State: Equatable {
        case running
        case exited
        case terminating
        case reaping
        case finished
    }

    private enum TerminationResult: Equatable {
        case initiated
        case reaped
        case inactive
    }

    let processIdentifier: pid_t
    private let lock = NSLock()
    private let onTermination: @Sendable () -> Void
    private let monitoringQueue: DispatchQueue
    private var deferReaping: Bool
    private var state = State.running
    private var status: Int32 = -1
    private var source: DispatchSourceProcess?

    var terminationStatus: Int32 {
        lock.lock()
        defer { lock.unlock() }
        return status
    }

    private init(
        processIdentifier: pid_t,
        monitoringQueue: DispatchQueue,
        deferReaping: Bool,
        onTermination: @escaping @Sendable () -> Void
    ) {
        self.processIdentifier = processIdentifier
        self.monitoringQueue = monitoringQueue
        self.deferReaping = deferReaping
        self.onTermination = onTermination
    }

    static func launch(
        configuredProcess: Process,
        stdinPipe: Pipe,
        stdoutPipe: Pipe,
        stderrPipe: Pipe,
        monitoringQueue: DispatchQueue = .global(qos: .userInitiated),
        deferReaping: Bool = false,
        onTermination: @escaping @Sendable () -> Void
    ) throws -> CancellableProcess {
        guard let executable = configuredProcess.executableURL?.path else {
            throw ProcessLaunchError.invalidExecutable
        }

        var actions: posix_spawn_file_actions_t?
        try check(posix_spawn_file_actions_init(&actions), operation: "initialize file actions")
        defer { posix_spawn_file_actions_destroy(&actions) }

        try addPipeActions(
            &actions,
            stdinPipe: stdinPipe,
            stdoutPipe: stdoutPipe,
            stderrPipe: stderrPipe
        )
        if let cwd = configuredProcess.currentDirectoryURL?.path {
            let result = cwd.withCString { path in
                posix_spawn_file_actions_addchdir_np(&actions, path)
            }
            try check(result, operation: "configure working directory")
        }

        var attributes: posix_spawnattr_t?
        try check(posix_spawnattr_init(&attributes), operation: "initialize spawn attributes")
        defer { posix_spawnattr_destroy(&attributes) }
        var defaultSignals = sigset_t()
        sigemptyset(&defaultSignals)
        for signal in [SIGTERM, SIGINT, SIGHUP, SIGQUIT, SIGPIPE] {
            sigaddset(&defaultSignals, signal)
        }
        var signalMask = sigset_t()
        sigemptyset(&signalMask)
        let flags = Int16(
            POSIX_SPAWN_SETPGROUP |
                POSIX_SPAWN_CLOEXEC_DEFAULT |
                POSIX_SPAWN_SETSIGDEF |
                POSIX_SPAWN_SETSIGMASK
        )
        try check(posix_spawnattr_setflags(&attributes, flags), operation: "configure spawn flags")
        try check(posix_spawnattr_setpgroup(&attributes, 0), operation: "configure process group")
        try check(posix_spawnattr_setsigdefault(&attributes, &defaultSignals), operation: "configure signal defaults")
        try check(posix_spawnattr_setsigmask(&attributes, &signalMask), operation: "configure signal mask")

        let arguments = [executable] + (configuredProcess.arguments ?? [])
        let environment = (configuredProcess.environment ?? ProcessInfo.processInfo.environment)
            .map { "\($0.key)=\($0.value)" }
        var pid: pid_t = 0
        let spawnResult = try withCStringArray(arguments) { argv in
            try withCStringArray(environment) { envp in
                executable.withCString { path in
                    posix_spawn(&pid, path, &actions, &attributes, argv, envp)
                }
            }
        }
        try check(spawnResult, operation: "spawn process")

        try? stdinPipe.fileHandleForReading.close()
        try? stdoutPipe.fileHandleForWriting.close()
        try? stderrPipe.fileHandleForWriting.close()

        let process = CancellableProcess(
            processIdentifier: pid,
            monitoringQueue: monitoringQueue,
            deferReaping: deferReaping,
            onTermination: onTermination
        )
        process.startMonitoring()
        return process
    }

    func terminate() -> Bool {
        switch requestTermination() {
        case .initiated: true
        case .reaped,
             .inactive: false
        }
    }

    func terminateProcessGroup() {
        _ = requestTermination()
    }

    func releaseReaping() {
        lock.lock()
        deferReaping = false
        guard state == .exited else {
            lock.unlock()
            return
        }
        state = .reaping
        lock.unlock()
        reapClaimedProcess()
    }

    private func requestTermination() -> TerminationResult {
        lock.lock()
        guard state == .running || state == .exited else {
            lock.unlock()
            return .inactive
        }
        if deferReaping {
            state = .terminating
            lock.unlock()
            scheduleTermination()
            return .initiated
        }
        var waitStatus: Int32 = 0
        var waitResult: pid_t
        repeat {
            waitResult = waitpid(processIdentifier, &waitStatus, WNOHANG)
        } while waitResult == -1 && errno == EINTR
        guard waitResult == 0 else {
            state = .finished
            status = waitResult == processIdentifier ? Self.exitCode(from: waitStatus) : -1
            let source = self.source
            self.source = nil
            lock.unlock()
            source?.cancel()
            Self.controlQueue.async { [self] in
                onTermination()
            }
            return .reaped
        }
        state = .terminating
        lock.unlock()
        scheduleTermination()
        return .initiated
    }

    private func scheduleTermination() {
        kill(-processIdentifier, SIGTERM)
        Self.controlQueue.asyncAfter(deadline: .now() + .milliseconds(200)) { [self] in
            lock.lock()
            guard state == .terminating else {
                lock.unlock()
                return
            }
            state = .reaping
            lock.unlock()
            kill(-processIdentifier, SIGKILL)
            reapClaimedProcess()
        }
    }

    private static func addPipeActions(
        _ actions: inout posix_spawn_file_actions_t?,
        stdinPipe: Pipe,
        stdoutPipe: Pipe,
        stderrPipe: Pipe
    ) throws {
        let descriptors = [
            (stdinPipe.fileHandleForReading.fileDescriptor, STDIN_FILENO),
            (stdoutPipe.fileHandleForWriting.fileDescriptor, STDOUT_FILENO),
            (stderrPipe.fileHandleForWriting.fileDescriptor, STDERR_FILENO),
        ]
        for (source, destination) in descriptors {
            try check(
                posix_spawn_file_actions_adddup2(&actions, source, destination),
                operation: "configure standard stream"
            )
        }
    }

    private static func check(_ result: Int32, operation: String) throws {
        guard result == 0 else {
            throw ProcessLaunchError.posix(operation, result)
        }
    }

    private static func withCStringArray<Result>(
        _ strings: [String],
        body: (UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) throws -> Result
    ) throws -> Result {
        var pointers: [UnsafeMutablePointer<CChar>?] = []
        defer {
            for pointer in pointers {
                free(pointer)
            }
        }
        for string in strings {
            guard !string.contains("\0") else {
                throw ProcessLaunchError.invalidCString
            }
            guard let pointer = strdup(string) else {
                throw ProcessLaunchError.allocationFailed
            }
            pointers.append(pointer)
        }
        pointers.append(nil)
        return try pointers.withUnsafeMutableBufferPointer { buffer in
            guard let baseAddress = buffer.baseAddress else {
                preconditionFailure("CString array must contain a terminator")
            }
            return try body(baseAddress)
        }
    }

    private func startMonitoring() {
        let source = DispatchSource.makeProcessSource(
            identifier: processIdentifier,
            eventMask: .exit,
            queue: monitoringQueue
        )
        source.setEventHandler { [self] in
            processDidExit()
        }
        lock.lock()
        self.source = source
        lock.unlock()
        source.resume()
    }

    private func processDidExit() {
        lock.lock()
        guard state == .running else {
            lock.unlock()
            return
        }
        if deferReaping {
            state = .exited
            let source = self.source
            self.source = nil
            lock.unlock()
            source?.cancel()
            return
        }
        state = .reaping
        lock.unlock()
        reapClaimedProcess()
    }

    private func reapClaimedProcess() {
        var waitStatus: Int32 = 0
        var result: pid_t
        repeat {
            result = waitpid(processIdentifier, &waitStatus, 0)
        } while result == -1 && errno == EINTR

        lock.lock()
        guard state == .reaping else {
            lock.unlock()
            return
        }
        state = .finished
        status = result == processIdentifier ? Self.exitCode(from: waitStatus) : -1
        let source = self.source
        self.source = nil
        lock.unlock()

        source?.cancel()
        onTermination()
    }

    private static func exitCode(from waitStatus: Int32) -> Int32 {
        let signal = waitStatus & 0x7F
        guard signal == 0 else { return signal }
        return (waitStatus >> 8) & 0xFF
    }
}

private enum ProcessLaunchError: Error, LocalizedError {
    case invalidExecutable
    case invalidCString
    case allocationFailed
    case posix(String, Int32)

    var errorDescription: String? {
        switch self {
        case .invalidExecutable:
            "missing executable"
        case .invalidCString:
            "arguments and environment cannot contain null bytes"
        case .allocationFailed:
            "could not allocate process arguments"
        case let .posix(operation, code):
            "\(operation): \(String(cString: strerror(code)))"
        }
    }
}

final class OutputReader: @unchecked Sendable {
    private enum CallbackEvent {
        case data(Data)
        case finish
    }

    private let lock = NSLock()
    private let handle: FileHandle
    private let box: OutputBox
    private let onData: (@Sendable (Data) -> Void)?
    private let onFinish: (@Sendable () -> Void)?
    private var finished = false
    private var callbackEvents: [CallbackEvent] = []
    private var deliveringCallbacks = false

    init(
        pipe: Pipe,
        box: OutputBox,
        onData: (@Sendable (Data) -> Void)? = nil,
        onFinish: (@Sendable () -> Void)? = nil
    ) {
        handle = pipe.fileHandleForReading
        self.box = box
        self.onData = onData
        self.onFinish = onFinish
    }

    func start() {
        handle.readabilityHandler = { [weak self] _ in
            self?.readAvailableData()
        }
    }

    func finish() {
        lock.lock()
        guard !finished else {
            lock.unlock()
            return
        }
        finished = true
        handle.readabilityHandler = nil

        let descriptor = handle.fileDescriptor
        let flags = fcntl(descriptor, F_GETFL)
        if flags >= 0 {
            _ = fcntl(descriptor, F_SETFL, flags | O_NONBLOCK)
        }
        var buffer = [UInt8](repeating: 0, count: 64 * 1024)
        while true {
            let bytesRead = buffer.withUnsafeMutableBytes { bytes in
                read(descriptor, bytes.baseAddress, bytes.count)
            }
            if bytesRead > 0 {
                callbackEvents.append(.data(Data(buffer.prefix(Int(bytesRead)))))
                continue
            }
            if bytesRead == -1, errno == EINTR {
                continue
            }
            break
        }
        try? handle.close()
        callbackEvents.append(.finish)
        let shouldDeliver = beginDeliveringCallbacks()
        lock.unlock()
        if shouldDeliver {
            deliverCallbacks()
        }
    }

    private func readAvailableData() {
        lock.lock()
        guard !finished else {
            lock.unlock()
            return
        }
        let data = handle.availableData
        if !data.isEmpty {
            callbackEvents.append(.data(data))
            let shouldDeliver = beginDeliveringCallbacks()
            lock.unlock()
            if shouldDeliver {
                deliverCallbacks()
            }
            return
        }
        finished = true
        handle.readabilityHandler = nil
        try? handle.close()
        callbackEvents.append(.finish)
        let shouldDeliver = beginDeliveringCallbacks()
        lock.unlock()
        if shouldDeliver {
            deliverCallbacks()
        }
    }

    private func beginDeliveringCallbacks() -> Bool {
        guard !deliveringCallbacks else { return false }
        deliveringCallbacks = true
        return true
    }

    private func deliverCallbacks() {
        while true {
            lock.lock()
            guard !callbackEvents.isEmpty else {
                deliveringCallbacks = false
                lock.unlock()
                return
            }
            let event = callbackEvents.removeFirst()
            lock.unlock()
            switch event {
            case let .data(data):
                box.append(data)
                onData?(data)
            case .finish:
                onFinish?()
            }
        }
    }
}

final class OutputBox: @unchecked Sendable {
    private let lock = NSLock()
    private let maximumBytes: Int
    private var data = Data()
    private(set) var overflow = false

    init(maximumBytes: Int = ExtensionCommandExecutor.maxOutputBytes) {
        self.maximumBytes = maximumBytes
    }

    func append(_ chunk: Data) {
        lock.lock()
        defer { lock.unlock() }
        if overflow {
            return
        }
        let remaining = maximumBytes - data.count
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

    func value() -> Data {
        lock.lock()
        defer { lock.unlock() }
        return data
    }
}
