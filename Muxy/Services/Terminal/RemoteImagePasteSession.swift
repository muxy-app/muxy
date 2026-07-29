import Foundation

struct RemoteImagePasteAttempt: Equatable, Sendable {
    let sessionID: String
    let imageID: String
    let surfaceGeneration: Int
}

@MainActor
final class RemoteImagePasteSession {
    typealias IdentifierGenerator = @MainActor () -> String

    private let identifierGenerator: IdentifierGenerator
    private var sessionID: String
    private(set) var isActive = false

    init(
        sessionID: String = UUID().uuidString,
        identifierGenerator: @escaping IdentifierGenerator = { UUID().uuidString }
    ) {
        self.sessionID = sessionID
        self.identifierGenerator = identifierGenerator
    }

    func begin(surfaceGeneration: Int) -> RemoteImagePasteAttempt {
        isActive = true
        return RemoteImagePasteAttempt(
            sessionID: sessionID,
            imageID: identifierGenerator(),
            surfaceGeneration: surfaceGeneration
        )
    }

    func permitsSideEffects(
        for attempt: RemoteImagePasteAttempt,
        surfaceGeneration: Int,
        hasLiveSurface: Bool,
        isCancelled: Bool
    ) -> Bool {
        guard !isCancelled, hasLiveSurface, isActive else { return false }
        guard attempt.surfaceGeneration == surfaceGeneration else { return false }
        return attempt.sessionID == sessionID
    }

    func takeActiveSessionForCleanup() -> String? {
        guard isActive else { return nil }
        let activeSessionID = sessionID
        sessionID = identifierGenerator()
        isActive = false
        return activeSessionID
    }
}
