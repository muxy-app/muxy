import Foundation
import os

private let providerDiscoveryLogger = Logger(subsystem: "app.muxy", category: "ProviderDiscovery")

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

    private static func runProcess(
        executablePath: String,
        arguments: [String],
        workingDirectory: String,
        timeout: TimeInterval
    ) async throws -> GitProcessResult {
        try await withThrowingTaskGroup(of: GitProcessResult.self) { group in
            group.addTask {
                try await GitProcessRunner.runResolved(
                    ResolvedLaunch(
                        executable: executablePath,
                        arguments: arguments,
                        workingDirectory: workingDirectory
                    ),
                    outputByteLimit: 64 * 1024
                )
            }
            group.addTask {
                try await Task.sleep(for: .seconds(timeout))
                throw ProviderDiscoveryError.timedOut(timeout)
            }
            defer { group.cancelAll() }
            guard let result = try await group.next() else {
                throw ProviderDiscoveryError.timedOut(timeout)
            }
            return result
        }
    }
}
