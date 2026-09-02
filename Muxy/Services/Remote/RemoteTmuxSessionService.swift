import Foundation
import os

private let remoteTmuxServiceLogger = Logger(subsystem: "app.muxy", category: "RemoteTmuxSession")

enum RemoteTmuxLookup: Equatable {
    case present
    case absent
    case unavailable
    case unknown
}

enum RemoteTmuxAvailability: Equatable {
    case available
    case unavailable
    case unknown
}

enum RemoteTmuxSessionService {
    static let availableMarker = "__MUXY_TMUX_AVAILABLE_7F07A1E4__"
    static let unavailableMarker = "__MUXY_TMUX_UNAVAILABLE_7F07A1E4__"
    static let presentMarker = "__MUXY_TMUX_PRESENT_7F07A1E4__"
    static let absentMarker = "__MUXY_TMUX_ABSENT_7F07A1E4__"
    static let unknownMarker = "__MUXY_TMUX_UNKNOWN_7F07A1E4__"
    static let lookupTimeout: TimeInterval = 12
    static let killTimeout: TimeInterval = 5
    static let killAttemptLimit = 3
    static let controlOutputByteLimit = 4096

    static func availability(for result: GitProcessResult) -> RemoteTmuxAvailability {
        guard result.status == 0, !result.truncated else { return .unknown }
        if containsMarker(availableMarker, in: result.stdout) {
            return .available
        }
        if containsMarker(unavailableMarker, in: result.stdout) {
            return .unavailable
        }
        if containsMarker(unknownMarker, in: result.stdout) {
            return .unknown
        }
        return .unknown
    }

    static func lookup(for result: GitProcessResult) -> RemoteTmuxLookup {
        guard result.status == 0, !result.truncated else { return .unknown }
        if containsMarker(presentMarker, in: result.stdout) {
            return .present
        }
        if containsMarker(absentMarker, in: result.stdout) {
            return .absent
        }
        if containsMarker(unavailableMarker, in: result.stdout) {
            return .unavailable
        }
        return .unknown
    }

    static func lookup(_ session: RemoteTmuxSession) async -> RemoteTmuxLookup {
        do {
            let result = try await SSHCommandRunner.run(
                destination: session.destination,
                remoteCommand: RemoteTmuxCommandBuilder.hasSessionCommand(for: session),
                outputByteLimit: controlOutputByteLimit,
                timeout: lookupTimeout
            )
            return lookup(for: result)
        } catch {
            remoteTmuxServiceLogger.info("remote tmux lookup failed for \(session.destination.host): \(error)")
            return .unknown
        }
    }

    static func kill(_ session: RemoteTmuxSession) async {
        for attempt in 1 ... killAttemptLimit {
            do {
                let result = try await SSHCommandRunner.run(
                    destination: session.destination,
                    remoteCommand: RemoteTmuxCommandBuilder.killSessionCommand(for: session),
                    outputByteLimit: controlOutputByteLimit,
                    timeout: killTimeout
                )
                if result.status == 0 {
                    return
                }
            } catch {
                if attempt == killAttemptLimit {
                    remoteTmuxServiceLogger.info("remote tmux cleanup failed for \(session.destination.host): \(error)")
                }
            }
            guard attempt < killAttemptLimit else { break }
            try? await Task.sleep(for: .milliseconds(250))
        }
        remoteTmuxServiceLogger.info("remote tmux cleanup did not confirm termination for \(session.destination.host)")
    }

    private static func containsMarker(_ marker: String, in output: String) -> Bool {
        output.trimmingCharacters(in: .whitespacesAndNewlines) == marker
    }
}
