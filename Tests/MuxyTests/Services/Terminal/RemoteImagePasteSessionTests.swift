import Testing

@testable import Muxy

@Suite("Remote image paste session")
@MainActor
struct RemoteImagePasteSessionTests {
    @Test("teardown claims every attempt activated before suspended work")
    func teardownClaimsActiveAttempt() {
        var identifiers = [
            "ABCDEF01-2345-6789-ABCD-EF0123456789",
            "FEDCBA98-7654-3210-FEDC-BA9876543210",
        ]
        let session = RemoteImagePasteSession(
            sessionID: "01234567-89AB-CDEF-0123-456789ABCDEF",
            identifierGenerator: { identifiers.removeFirst() }
        )

        let attempt = session.begin(surfaceGeneration: 4)

        #expect(session.isActive)
        #expect(session.permitsSideEffects(
            for: attempt,
            surfaceGeneration: 4,
            hasLiveSurface: true,
            isCancelled: false
        ))
        #expect(session.takeActiveSessionForCleanup() == attempt.sessionID)
        #expect(!session.permitsSideEffects(
            for: attempt,
            surfaceGeneration: 4,
            hasLiveSurface: true,
            isCancelled: false
        ))
    }

    @Test("cancellation generation and live surface all gate side effects")
    func sideEffectGates() {
        let session = RemoteImagePasteSession(
            sessionID: "01234567-89AB-CDEF-0123-456789ABCDEF",
            identifierGenerator: { "ABCDEF01-2345-6789-ABCD-EF0123456789" }
        )
        let attempt = session.begin(surfaceGeneration: 2)

        #expect(!session.permitsSideEffects(
            for: attempt,
            surfaceGeneration: 2,
            hasLiveSurface: true,
            isCancelled: true
        ))
        #expect(!session.permitsSideEffects(
            for: attempt,
            surfaceGeneration: 3,
            hasLiveSurface: true,
            isCancelled: false
        ))
        #expect(!session.permitsSideEffects(
            for: attempt,
            surfaceGeneration: 2,
            hasLiveSurface: false,
            isCancelled: false
        ))
    }
}
