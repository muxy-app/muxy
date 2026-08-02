import Foundation

struct RemoteUploadAttempt: Equatable, Sendable {
    let sessionID: String
    let uploadID: String
    let surfaceGeneration: Int
    let destination: SSHDestination
}

@MainActor
final class RemoteUploadSession {
    typealias IdentifierGenerator = @MainActor () -> String

    private let identifierGenerator: IdentifierGenerator
    private var sessionID: String
    private var activeDestination: SSHDestination?
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
        activeDestination = destination
        return RemoteUploadAttempt(
            sessionID: sessionID,
            uploadID: identifierGenerator(),
            surfaceGeneration: surfaceGeneration,
            destination: destination
        )
    }

    func permitsSideEffects(
        for attempt: RemoteUploadAttempt,
        surfaceGeneration: Int,
        hasLiveSurface: Bool,
        isCancelled: Bool
    ) -> Bool {
        guard !isCancelled, hasLiveSurface, isActive else { return false }
        guard attempt.surfaceGeneration == surfaceGeneration else { return false }
        return attempt.sessionID == sessionID
    }

    func takeActiveSessionForCleanup() -> (sessionID: String, destination: SSHDestination)? {
        guard isActive, let destination = activeDestination else { return nil }
        let activeSessionID = sessionID
        sessionID = identifierGenerator()
        activeDestination = nil
        isActive = false
        return (activeSessionID, destination)
    }
}
