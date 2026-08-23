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

enum SSHConnectionProbeResult: Equatable {
    case succeeded
    case failed
    case superseded
}

@MainActor
@Observable
final class SSHConnectionService {
    static let shared = SSHConnectionService()

    private(set) var states: [SSHConnectionKey: SSHConnectionState] = [:]
    private var probeRequestIDs: [SSHConnectionKey: UUID] = [:]

    func state(for destination: SSHDestination) -> SSHConnectionState {
        states[destination.connectionKey] ?? .disconnected
    }

    func reset(destination: SSHDestination) {
        let key = destination.connectionKey
        probeRequestIDs.removeValue(forKey: key)
        states[key] = .disconnected
    }

    @discardableResult
    func test(destination: SSHDestination) async -> SSHConnectionProbeResult {
        await probe(destination: destination, busyState: .testing, batch: true)
    }

    @discardableResult
    func connect(destination: SSHDestination) async -> SSHConnectionProbeResult {
        await probe(destination: destination, busyState: .connecting, batch: false)
    }

    private func probe(
        destination: SSHDestination,
        busyState: SSHConnectionState,
        batch: Bool
    ) async -> SSHConnectionProbeResult {
        let key = destination.connectionKey
        let requestID = UUID()
        probeRequestIDs[key] = requestID
        states[key] = busyState
        defer { clearProbeRequest(key: key, requestID: requestID) }
        do {
            let remoteCommand = destination.remoteSessionMode == .tmux
                ? RemoteTmuxCommandBuilder.availabilityCommand()
                : "echo \(Self.marker)"
            let result = try await SSHCommandRunner.run(
                destination: destination,
                remoteCommand: remoteCommand,
                batch: batch,
                outputByteLimit: destination.remoteSessionMode == .tmux
                    ? RemoteTmuxSessionService.controlOutputByteLimit
                    : nil
            )
            guard probeRequestIDs[key] == requestID else { return .superseded }
            guard !Task.isCancelled else {
                states[key] = .disconnected
                return .superseded
            }
            if destination.remoteSessionMode == .tmux,
               RemoteTmuxSessionService.availability(for: result) == .unavailable
            {
                states[key] = .failed("tmux is not installed or unavailable on the remote host.")
                return .failed
            }
            guard result.status == 0,
                  destination.remoteSessionMode == .tmux
                  ? RemoteTmuxSessionService.availability(for: result) == .available
                  : result.stdout.contains(Self.marker)
            else {
                states[key] = .failed(Self.failureMessage(result))
                return .failed
            }
            states[key] = .connected
            return .succeeded
        } catch {
            guard probeRequestIDs[key] == requestID else { return .superseded }
            guard !Task.isCancelled else {
                states[key] = .disconnected
                return .superseded
            }
            logger.error("SSH probe failed for \(destination.host): \(error)")
            states[key] = .failed(error.localizedDescription)
            return .failed
        }
    }

    private func clearProbeRequest(key: SSHConnectionKey, requestID: UUID) {
        guard probeRequestIDs[key] == requestID else { return }
        probeRequestIDs.removeValue(forKey: key)
    }

    private static let marker = "MUXY_SSH_OK"

    private static func failureMessage(_ result: GitProcessResult) -> String {
        let stderr = result.stderr.trimmingCharacters(in: .whitespacesAndNewlines)
        guard stderr.isEmpty else { return stderr }
        return "Connection failed (exit \(result.status))."
    }
}
