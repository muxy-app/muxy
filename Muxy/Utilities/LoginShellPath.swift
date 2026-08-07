import Darwin
import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "LoginShellPath")

struct LoginShellEnvironmentValues: Equatable, Sendable {
    let path: String
    let copilotHome: String?
}

final class LoginShellPath: @unchecked Sendable {
    static let shared = LoginShellPath()
    static let shellArguments = [
        "-l",
        "-i",
        "-c",
        "printf '__MUXY_PATH_START__'; /usr/bin/printenv PATH; printf '__MUXY_PATH_END__'; "
            + "printf '__MUXY_COPILOT_HOME_START__'; /usr/bin/printenv COPILOT_HOME; "
            + "printf '__MUXY_COPILOT_HOME_END__'",
    ]

    private static let pathStartMarker = "__MUXY_PATH_START__"
    private static let pathEndMarker = "__MUXY_PATH_END__"
    private static let copilotHomeStartMarker = "__MUXY_COPILOT_HOME_START__"
    private static let copilotHomeEndMarker = "__MUXY_COPILOT_HOME_END__"
    private static let shellOutputByteLimit = 262_144

    private let lock = NSLock()
    private var cached: String?
    private var cachedCopilotHome: String?
    private var environmentHydrated = false

    init() {}

    static var current: String { shared.value }
    static var currentCopilotHome: String? { shared.copilotHome }

    static var defaultPath: String {
        ProcessInfo.processInfo.environment["PATH"] ?? "/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
    }

    static var defaultCopilotHome: String? {
        guard let value = ProcessInfo.processInfo.environment["COPILOT_HOME"], !value.isEmpty else { return nil }
        return value
    }

    static func hydrateInBackground() {
        shared.hydrateInBackground()
    }

    static func hydrate() async {
        await shared.hydrateEnvironment()
    }

    var value: String {
        lock.withLock { cached ?? Self.defaultPath }
    }

    var copilotHome: String? {
        lock.withLock { environmentHydrated ? cachedCopilotHome : Self.defaultCopilotHome }
    }

    func hydrate(readFromLoginShell: @escaping @Sendable () -> String? = LoginShellPath.readFromLoginShell) async {
        let resolved = await Task.detached(priority: .utility) {
            readFromLoginShell()
        }.value
        guard let resolved, !resolved.isEmpty else {
            logger.info("Login shell PATH lookup yielded no value; keeping launchd PATH")
            return
        }
        lock.withLock {
            cached = resolved
        }
        logger.info("Hydrated PATH from login shell")
    }

    func hydrateEnvironment(
        readFromLoginShell: @escaping @Sendable () -> LoginShellEnvironmentValues? = LoginShellPath
            .readEnvironmentFromLoginShell
    ) async {
        let resolved = await Task.detached(priority: .utility) {
            readFromLoginShell()
        }.value
        guard let resolved else {
            logger.info("Login shell environment lookup yielded no value; keeping launch environment")
            return
        }
        lock.withLock {
            cached = resolved.path
            cachedCopilotHome = resolved.copilotHome
            environmentHydrated = true
        }
        logger.info("Hydrated environment from login shell")
    }

    private func hydrateInBackground() {
        Task.detached(priority: .utility) { [self] in
            await hydrateEnvironment()
        }
    }

    private static func readFromLoginShell() -> String? {
        readPath(shellPath: UserShell.path(), arguments: shellArguments)
    }

    private static func readEnvironmentFromLoginShell() -> LoginShellEnvironmentValues? {
        readEnvironment(shellPath: UserShell.path(), arguments: shellArguments)
    }

    static func readPath(
        shellPath: String,
        arguments: [String],
        timeout: DispatchTimeInterval = .seconds(3)
    ) -> String? {
        guard let output = readShellOutput(shellPath: shellPath, arguments: arguments, timeout: timeout) else {
            return nil
        }
        return extractPath(from: output)
    }

    static func readEnvironment(
        shellPath: String,
        arguments: [String],
        timeout: DispatchTimeInterval = .seconds(3)
    ) -> LoginShellEnvironmentValues? {
        guard let output = readShellOutput(shellPath: shellPath, arguments: arguments, timeout: timeout) else {
            return nil
        }
        return extractEnvironment(from: output)
    }

    private static func readShellOutput(
        shellPath: String,
        arguments: [String],
        timeout: DispatchTimeInterval
    ) -> Data? {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: shellPath)
        process.arguments = arguments

        let exited = DispatchSemaphore(value: 0)
        process.terminationHandler = { _ in exited.signal() }

        let stdout = Pipe()
        let stderr = Pipe()
        process.standardOutput = stdout
        process.standardError = stderr

        do {
            try process.run()
        } catch {
            logger.error("Failed to launch login shell: \(error.localizedDescription)")
            return nil
        }

        let deadline = DispatchTime.now() + timeout
        let stdoutReader = BoundedPipeReader(
            handle: stdout.fileHandleForReading,
            byteLimit: shellOutputByteLimit
        )
        let stderrReader = BoundedPipeReader(
            handle: stderr.fileHandleForReading,
            byteLimit: shellOutputByteLimit
        )
        stdoutReader.start()
        stderrReader.start()

        if exited.wait(timeout: deadline) == .timedOut {
            terminate(process)
            process.waitUntilExit()
            stdoutReader.cancel()
            stderrReader.cancel()
            return nil
        }
        process.waitUntilExit()

        guard let stdoutData = stdoutReader.wait(until: deadline),
              stderrReader.wait(until: deadline) != nil
        else {
            stdoutReader.cancel()
            stderrReader.cancel()
            return nil
        }
        guard process.terminationStatus == 0 else { return nil }

        return stdoutData
    }

    private static func terminate(_ process: Process) {
        process.terminate()
        let deadline = ContinuousClock.now + .milliseconds(500)
        while process.isRunning, ContinuousClock.now < deadline {
            usleep(10000)
        }
        guard process.isRunning else { return }
        kill(process.processIdentifier, SIGKILL)
    }

    static func extractPath(from shellOutputData: Data) -> String? {
        guard let output = decodedShellOutput(from: shellOutputData) else { return nil }
        return extractPath(from: output)
    }

    static func extractPath(from shellOutput: String) -> String? {
        extractedValue(
            from: shellOutput,
            startMarker: pathStartMarker,
            endMarker: pathEndMarker
        )
    }

    static func extractEnvironment(from shellOutputData: Data) -> LoginShellEnvironmentValues? {
        guard let output = decodedShellOutput(from: shellOutputData) else { return nil }
        return extractEnvironment(from: output)
    }

    static func extractEnvironment(from shellOutput: String) -> LoginShellEnvironmentValues? {
        guard let path = extractPath(from: shellOutput),
              shellOutput.range(of: copilotHomeStartMarker, options: .backwards) != nil,
              shellOutput.range(of: copilotHomeEndMarker, options: .backwards) != nil
        else { return nil }
        return LoginShellEnvironmentValues(
            path: path,
            copilotHome: extractedValue(
                from: shellOutput,
                startMarker: copilotHomeStartMarker,
                endMarker: copilotHomeEndMarker
            )
        )
    }

    private static func decodedShellOutput(from data: Data) -> String? {
        let bytes = Array(data)
        guard let validStart = bytes.firstIndex(where: { $0 & 0xC0 != 0x80 }) else { return nil }
        return String(bytes: bytes[validStart...], encoding: .utf8)
    }

    private static func extractedValue(
        from output: String,
        startMarker: String,
        endMarker: String
    ) -> String? {
        guard let start = output.range(of: startMarker, options: .backwards) else { return nil }
        let outputAfterStart = output[start.upperBound...]
        guard let end = outputAfterStart.range(of: endMarker) else { return nil }
        let value = outputAfterStart[..<end.lowerBound]
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return value.isEmpty ? nil : value
    }
}

private final class BoundedPipeReader: @unchecked Sendable {
    private let handle: FileHandle
    private let byteLimit: Int
    private let lock = NSLock()
    private let semaphore = DispatchSemaphore(value: 0)
    private var data = Data()
    private var finished = false

    init(handle: FileHandle, byteLimit: Int) {
        self.handle = handle
        self.byteLimit = byteLimit
    }

    func start() {
        handle.readabilityHandler = { [weak self] _ in
            self?.readAvailableData()
        }
    }

    func wait(until deadline: DispatchTime) -> Data? {
        guard semaphore.wait(timeout: deadline) == .success else { return nil }
        return lock.withLock { data }
    }

    func cancel() {
        finish()
    }

    private func readAvailableData() {
        lock.lock()
        guard !finished else {
            lock.unlock()
            return
        }
        let chunk = handle.availableData
        guard !chunk.isEmpty else {
            lock.unlock()
            finish()
            return
        }
        if chunk.count >= byteLimit {
            data = Data(chunk.suffix(byteLimit))
            lock.unlock()
            return
        }
        let overflow = data.count + chunk.count - byteLimit
        if overflow > 0 {
            data.removeFirst(overflow)
        }
        data.append(chunk)
        lock.unlock()
    }

    private func finish() {
        lock.lock()
        guard !finished else {
            lock.unlock()
            return
        }
        finished = true
        handle.readabilityHandler = nil
        try? handle.close()
        lock.unlock()
        semaphore.signal()
    }
}
