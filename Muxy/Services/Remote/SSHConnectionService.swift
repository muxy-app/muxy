import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "SSHConnectionService")

enum SSHConnectionState: Equatable {
    case disconnected
    case testing
    case connecting
    case connected
    case failed(String)

    var isBusy: Bool {
        self == .testing || self == .connecting
    }
}

@MainActor
@Observable
final class SSHConnectionService {
    static let shared = SSHConnectionService()

    private(set) var states: [SSHConnectionKey: SSHConnectionState] = [:]

    func state(for destination: SSHDestination) -> SSHConnectionState {
        states[destination.connectionKey] ?? .disconnected
    }

    func reset(destination: SSHDestination) {
        states[destination.connectionKey] = .disconnected
    }

    @discardableResult
    func test(destination: SSHDestination) async -> Bool {
        await probe(destination: destination, busyState: .testing, batch: true)
    }

    @discardableResult
    func connect(destination: SSHDestination) async -> Bool {
        await probe(destination: destination, busyState: .connecting, batch: false)
    }

    private func probe(destination: SSHDestination, busyState: SSHConnectionState, batch: Bool) async -> Bool {
        let key = destination.connectionKey
        states[key] = busyState
        do {
            let result = try await SSHCommandRunner.run(
                destination: destination,
                remoteCommand: "echo \(Self.marker)",
                batch: batch
            )
            guard result.status == 0, result.stdout.contains(Self.marker) else {
                states[key] = .failed(Self.failureMessage(result))
                return false
            }
            states[key] = .connected
            return true
        } catch {
            logger.error("SSH probe failed for \(destination.host): \(error)")
            states[key] = .failed(error.localizedDescription)
            return false
        }
    }

    private static let marker = "MUXY_SSH_OK"

    private static func failureMessage(_ result: GitProcessResult) -> String {
        let stderr = result.stderr.trimmingCharacters(in: .whitespacesAndNewlines)
        guard stderr.isEmpty else { return stderr }
        return "Connection failed (exit \(result.status))."
    }
}
