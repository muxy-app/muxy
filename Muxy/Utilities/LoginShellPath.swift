import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "LoginShellPath")

final class LoginShellPath: @unchecked Sendable {
    static let shared = LoginShellPath()
    static let shellArguments = [
        "-l",
        "-i",
        "-c",
        "printf '__MUXY_PATH_START__'; /usr/bin/printenv PATH; printf '__MUXY_PATH_END__'",
    ]

    private static let pathStartMarker = "__MUXY_PATH_START__"
    private static let pathEndMarker = "__MUXY_PATH_END__"
    private static let shellOutputByteLimit = 262_144

    private let lock = NSLock()
    private var cached: String?

    init() {}

    static var current: String { shared.value }

    static var defaultPath: String {
        ProcessInfo.processInfo.environment["PATH"] ?? "/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
    }

    static func hydrateInBackground() {
        shared.hydrateInBackground()
    }

    static func hydrate() async {
        await shared.hydrate()
    }

    var value: String {
        lock.lock()
        defer { lock.unlock() }
        return cached ?? Self.defaultPath
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

    private func hydrateInBackground() {
        Task.detached(priority: .utility) { [self] in
            await hydrate()
        }
    }

    private static func readFromLoginShell() -> String? {
        readPath(shellPath: UserShell.path(), arguments: shellArguments)
    }

    static func readPath(
        shellPath: String,
        arguments: [String],
        timeout: DispatchTimeInterval = .seconds(3)
    ) -> String? {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: shellPath)
        process.arguments = arguments

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

        let waiter = DispatchSemaphore(value: 0)
        DispatchQueue.global(qos: .utility).async {
            process.waitUntilExit()
            waiter.signal()
        }
        if waiter.wait(timeout: deadline) == .timedOut {
            if process.isRunning {
                process.terminate()
            }
            stdoutReader.cancel()
            stderrReader.cancel()
            return nil
        }

        guard let stdoutData = stdoutReader.wait(until: deadline),
              stderrReader.wait(until: deadline) != nil
        else {
            stdoutReader.cancel()
            stderrReader.cancel()
            return nil
        }
        guard process.terminationStatus == 0 else { return nil }

        return extractPath(from: stdoutData)
    }

    static func extractPath(from shellOutputData: Data) -> String? {
        let bytes = Array(shellOutputData)
        guard let validStart = bytes.firstIndex(where: { $0 & 0xC0 != 0x80 }),
              let output = String(bytes: bytes[validStart...], encoding: .utf8)
        else { return nil }
        return extractPath(from: output)
    }

    static func extractPath(from shellOutput: String) -> String? {
        guard let start = shellOutput.range(of: pathStartMarker, options: .backwards) else { return nil }
        let outputAfterStart = shellOutput[start.upperBound...]
        guard let end = outputAfterStart.range(of: pathEndMarker) else { return nil }
        let path = outputAfterStart[..<end.lowerBound]
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return path.isEmpty ? nil : path
    }
}

private final class BoundedPipeReader: @unchecked Sendable {
    private let handle: FileHandle
    private let byteLimit: Int
    private let lock = NSLock()
    private let semaphore = DispatchSemaphore(value: 0)
    private var data = Data()

    init(handle: FileHandle, byteLimit: Int) {
        self.handle = handle
        self.byteLimit = byteLimit
    }

    func start() {
        DispatchQueue.global(qos: .utility).async { [self] in
            var collected = Data()
            while true {
                let chunk = (try? handle.read(upToCount: 65536)) ?? Data()
                if chunk.isEmpty {
                    break
                }
                if chunk.count >= byteLimit {
                    collected = Data(chunk.suffix(byteLimit))
                    continue
                }
                let overflow = collected.count + chunk.count - byteLimit
                if overflow > 0 {
                    collected.removeFirst(overflow)
                }
                collected.append(chunk)
            }
            lock.withLock {
                data = collected
            }
            semaphore.signal()
        }
    }

    func wait(until deadline: DispatchTime) -> Data? {
        guard semaphore.wait(timeout: deadline) == .success else { return nil }
        return lock.withLock { data }
    }

    func cancel() {
        try? handle.close()
    }
}
