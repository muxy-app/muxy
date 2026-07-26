import Foundation
import os

private let providerDiscoveryLogger = Logger(subsystem: "app.muxy", category: "ProviderDiscovery")
private let providerDiscoveryOutputByteLimit = 64 * 1024
private let providerDiscoveryProcessQueue = DispatchQueue(
    label: "app.muxy.provider-discovery",
    qos: .utility,
    attributes: .concurrent
)

enum ProviderDiscoveryState: Equatable {
    case ready
    case warning(String)
    case failed(String)
}

struct ProviderDiscoveryDetails: Equatable {
    let version: String?
    let state: ProviderDiscoveryState
}

struct ProviderDiscoverySnapshot: Equatable {
    let executablePath: String?
    let version: String?
    let state: ProviderDiscoveryState
}

protocol AIProviderDiscoveryIntegration {
    var discoveryArguments: [String] { get }
    var discoveryWorkingDirectory: String { get }

    func discoveryDetails(from output: String) -> ProviderDiscoveryDetails
}

enum ProviderDiscoveryError: LocalizedError {
    case commandFailed(Int32, String)
    case outputTruncated
    case timedOut(TimeInterval)

    var errorDescription: String? {
        switch self {
        case let .commandFailed(status, detail):
            detail.isEmpty ? "Discovery exited with status \(status)" : detail
        case .outputTruncated:
            "Discovery output exceeded the size limit"
        case let .timedOut(timeout):
            "Discovery timed out after \(Int(timeout))s"
        }
    }
}

@MainActor
struct ProviderDiscoveryService {
    typealias Runner = @Sendable (
        _ executablePath: String,
        _ arguments: [String],
        _ workingDirectory: String,
        _ timeout: TimeInterval
    ) async throws -> GitProcessResult

    static let defaultTimeout: TimeInterval = 5

    private let health: HookHealthStore
    private let timeout: TimeInterval
    private let runner: Runner

    init(
        health: HookHealthStore = .shared,
        timeout: TimeInterval = ProviderDiscoveryService.defaultTimeout,
        runner: @escaping Runner = ProviderDiscoveryService.runProcess
    ) {
        self.health = health
        self.timeout = timeout
        self.runner = runner
    }

    func discover(_ provider: AIProviderIntegration) async {
        guard provider.isEnabled,
              let launchProvider = provider as? any AIAgentLaunchProvider,
              let discoveryProvider = provider as? any AIProviderDiscoveryIntegration
        else { return }

        guard let executablePath = launchProvider.agentCLIExecutablePath() else {
            record(
                provider: provider,
                snapshot: ProviderDiscoverySnapshot(
                    executablePath: nil,
                    version: nil,
                    state: .failed("CLI executable not found")
                )
            )
            return
        }

        do {
            let result = try await runner(
                executablePath,
                discoveryProvider.discoveryArguments,
                discoveryProvider.discoveryWorkingDirectory,
                timeout
            )
            guard result.status == 0 else {
                throw ProviderDiscoveryError.commandFailed(
                    result.status,
                    result.stderr.trimmingCharacters(in: .whitespacesAndNewlines)
                )
            }
            guard !result.truncated else { throw ProviderDiscoveryError.outputTruncated }
            let details = discoveryProvider.discoveryDetails(from: result.stdout)
            record(
                provider: provider,
                snapshot: ProviderDiscoverySnapshot(
                    executablePath: executablePath,
                    version: details.version,
                    state: details.state
                )
            )
        } catch {
            record(
                provider: provider,
                snapshot: ProviderDiscoverySnapshot(
                    executablePath: executablePath,
                    version: nil,
                    state: .failed(error.localizedDescription)
                )
            )
        }
    }

    private func record(provider: AIProviderIntegration, snapshot: ProviderDiscoverySnapshot) {
        health.noteDiscovery(providerID: provider.id, snapshot: snapshot)
        let path = snapshot.executablePath ?? "not found"
        let version = snapshot.version ?? "unknown"
        switch snapshot.state {
        case .ready:
            providerDiscoveryLogger.info(
                "provider=\(provider.id, privacy: .public) state=ready version=\(version, privacy: .public)"
            )
        case let .warning(message):
            providerDiscoveryLogger.warning(
                "provider=\(provider.id, privacy: .public) state=warning version=\(version, privacy: .public) detail=\(message, privacy: .public)"
            )
        case let .failed(message):
            providerDiscoveryLogger.error(
                "provider=\(provider.id, privacy: .public) state=failed version=\(version, privacy: .public) detail=\(message, privacy: .public)"
            )
        }
        providerDiscoveryLogger.debug(
            "provider=\(provider.id, privacy: .public) executable=\(path, privacy: .private)"
        )
    }

    static func runProcess(
        executablePath: String,
        arguments: [String],
        workingDirectory: String,
        timeout: TimeInterval
    ) async throws -> GitProcessResult {
        try await withCheckedThrowingContinuation { continuation in
            providerDiscoveryProcessQueue.async {
                do {
                    try continuation.resume(returning: runProcessSynchronously(
                        executablePath: executablePath,
                        arguments: arguments,
                        workingDirectory: workingDirectory,
                        timeout: timeout
                    ))
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }

    nonisolated private static func runProcessSynchronously(
        executablePath: String,
        arguments: [String],
        workingDirectory: String,
        timeout: TimeInterval
    ) throws -> GitProcessResult {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: executablePath)
        process.arguments = arguments
        process.currentDirectoryURL = URL(fileURLWithPath: workingDirectory)
        process.environment = ProcessInfo.processInfo.environment

        let stdoutPipe = Pipe()
        let stderrPipe = Pipe()
        process.standardOutput = stdoutPipe
        process.standardError = stderrPipe

        let stdout = ProviderDiscoveryOutputCollector(byteLimit: providerDiscoveryOutputByteLimit)
        let stderr = ProviderDiscoveryOutputCollector(byteLimit: providerDiscoveryOutputByteLimit)
        stdout.start(reading: stdoutPipe.fileHandleForReading)
        stderr.start(reading: stderrPipe.fileHandleForReading)

        let exited = DispatchSemaphore(value: 0)
        process.terminationHandler = { _ in exited.signal() }

        do {
            try process.run()
        } catch {
            stdout.stop(reading: stdoutPipe.fileHandleForReading)
            stderr.stop(reading: stderrPipe.fileHandleForReading)
            throw error
        }

        let timedOut = exited.wait(timeout: .now() + timeout) == .timedOut
        if timedOut {
            terminate(process, exited: exited)
        }
        process.waitUntilExit()

        if !timedOut {
            let drainDeadline = DispatchTime.now() + 0.5
            stdout.waitForCompletion(until: drainDeadline)
            stderr.waitForCompletion(until: drainDeadline)
        }
        stdout.stop(reading: stdoutPipe.fileHandleForReading)
        stderr.stop(reading: stderrPipe.fileHandleForReading)

        if timedOut {
            throw ProviderDiscoveryError.timedOut(timeout)
        }
        let stdoutData = stdout.data
        return GitProcessResult(
            status: process.terminationStatus,
            stdout: String(data: stdoutData, encoding: .utf8) ?? "",
            stdoutData: stdoutData,
            stderr: String(data: stderr.data, encoding: .utf8) ?? "",
            truncated: stdout.truncated || stderr.truncated
        )
    }

    nonisolated private static func terminate(_ process: Process, exited: DispatchSemaphore) {
        guard process.isRunning else { return }
        process.terminate()
        guard exited.wait(timeout: .now() + 0.5) == .timedOut, process.isRunning else { return }
        kill(process.processIdentifier, SIGKILL)
    }
}

private final class ProviderDiscoveryOutputCollector: @unchecked Sendable {
    private let lock = NSLock()
    private let completed = DispatchSemaphore(value: 0)
    private let byteLimit: Int
    private var storage = Data()
    private var didTruncate = false
    private var didComplete = false

    init(byteLimit: Int) {
        self.byteLimit = byteLimit
    }

    var data: Data {
        lock.withLock { storage }
    }

    var truncated: Bool {
        lock.withLock { didTruncate }
    }

    func start(reading handle: FileHandle) {
        handle.readabilityHandler = { [weak self] handle in
            guard let self else { return }
            let chunk = handle.availableData
            guard !chunk.isEmpty else {
                complete()
                return
            }
            lock.withLock {
                let remaining = byteLimit - storage.count
                if remaining > 0 {
                    storage.append(chunk.prefix(remaining))
                }
                if chunk.count > remaining {
                    didTruncate = true
                }
            }
        }
    }

    func waitForCompletion(until deadline: DispatchTime) {
        _ = completed.wait(timeout: deadline)
    }

    func stop(reading handle: FileHandle) {
        handle.readabilityHandler = nil
        try? handle.close()
        complete()
    }

    private func complete() {
        let shouldSignal = lock.withLock {
            guard !didComplete else { return false }
            didComplete = true
            return true
        }
        if shouldSignal {
            completed.signal()
        }
    }
}
