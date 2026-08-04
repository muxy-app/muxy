import Foundation

struct RemoteUploadAttempt: Equatable, Sendable {
    let sessionID: String
    let uploadID: String
    let surfaceGeneration: Int
    let destination: SSHDestination
}

struct RemoteUploadCleanup: Equatable, Sendable {
    let sessionID: String
    let destinations: Set<SSHDestination>
}

extension TerminalUploadAttempt {
    func matches(destination: SSHDestination?) -> Bool {
        switch self {
        case .local:
            destination == nil
        case let .remote(attempt):
            attempt.destination == destination
        }
    }
}

@MainActor
final class RemoteUploadSession {
    typealias IdentifierGenerator = @MainActor () -> String

    private let identifierGenerator: IdentifierGenerator
    private var sessionID: String
    private var activeDestinations: Set<SSHDestination> = []
    private(set) var isActive = false

    init(
        sessionID: String = UUID().uuidString,
        identifierGenerator: @escaping IdentifierGenerator = { UUID().uuidString }
    ) {
        self.sessionID = sessionID
        self.identifierGenerator = identifierGenerator
    }

    func begin(surfaceGeneration: Int, destination: SSHDestination) -> RemoteUploadAttempt {
        isActive = true
        activeDestinations.insert(destination)
        return RemoteUploadAttempt(
            sessionID: sessionID,
            uploadID: identifierGenerator(),
            surfaceGeneration: surfaceGeneration,
            destination: destination
        )
    }

    func permitsSideEffects(
        for attempt: RemoteUploadAttempt,
        currentDestination: SSHDestination?,
        surfaceGeneration: Int,
        hasLiveSurface: Bool,
        isCancelled: Bool
    ) -> Bool {
        guard !isCancelled, hasLiveSurface, isActive else { return false }
        guard attempt.surfaceGeneration == surfaceGeneration else { return false }
        guard attempt.destination == currentDestination else { return false }
        return attempt.sessionID == sessionID && activeDestinations.contains(attempt.destination)
    }

    func takeActiveSessionForCleanup() -> RemoteUploadCleanup? {
        guard isActive, !activeDestinations.isEmpty else { return nil }
        let cleanup = RemoteUploadCleanup(
            sessionID: sessionID,
            destinations: activeDestinations
        )
        sessionID = identifierGenerator()
        activeDestinations = []
        isActive = false
        return cleanup
    }
}
