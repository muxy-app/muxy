import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "LoginShellPath")

final class LoginShellPath: @unchecked Sendable {
    static let shared = LoginShellPath()

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
        let shellPath = ProcessInfo.processInfo.environment["SHELL"] ?? "/bin/zsh"
        let process = Process()
        process.executableURL = URL(fileURLWithPath: shellPath)
        process.arguments = ["-l", "-c", "printf %s \"$PATH\""]

        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = Pipe()

        do {
            try process.run()
        } catch {
            logger.error("Failed to launch login shell: \(error.localizedDescription)")
            return nil
        }

        let deadline = DispatchTime.now() + .seconds(3)
        let waiter = DispatchSemaphore(value: 0)
        DispatchQueue.global(qos: .utility).async {
            process.waitUntilExit()
            waiter.signal()
        }
        if waiter.wait(timeout: deadline) == .timedOut {
            if process.isRunning {
                process.terminate()
            }
            return nil
        }

        guard process.terminationStatus == 0,
              let data = try? pipe.fileHandleForReading.readToEnd(),
              let text = String(data: data, encoding: .utf8)
        else { return nil }

        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }
}
