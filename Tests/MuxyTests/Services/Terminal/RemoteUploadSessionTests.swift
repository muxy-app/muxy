import Testing

@testable import Muxy

@Suite("Remote upload session")
@MainActor
struct RemoteUploadSessionTests {
    @Test("teardown claims every attempt activated before suspended work")
    func teardownClaimsActiveAttempt() {
        var identifiers = [
            "ABCDEF01-2345-6789-ABCD-EF0123456789",
            "FEDCBA98-7654-3210-FEDC-BA9876543210",
        ]
        let session = RemoteUploadSession(
            sessionID: "01234567-89AB-CDEF-0123-456789ABCDEF",
            identifierGenerator: { identifiers.removeFirst() }
        )

        let attempt = session.begin(surfaceGeneration: 4, destination: SSHDestination(host: "example.com"))

        #expect(session.isActive)
        #expect(session.permitsSideEffects(
            for: attempt,
            currentDestination: attempt.destination,
            surfaceGeneration: 4,
            hasLiveSurface: true,
            isCancelled: false
        ))
        let cleanup = session.takeActiveSessionForCleanup()
        #expect(cleanup?.sessionID == attempt.sessionID)
        #expect(cleanup?.destinations == [attempt.destination])
        #expect(!session.permitsSideEffects(
            for: attempt,
            currentDestination: attempt.destination,
            surfaceGeneration: 4,
            hasLiveSurface: true,
            isCancelled: false
        ))
    }

    @Test("cleanup retains every destination used by the session")
    func cleanupRetainsEveryDestination() throws {
        var identifiers = [
            "ABCDEF01-2345-6789-ABCD-EF0123456789",
            "FEDCBA98-7654-3210-FEDC-BA9876543210",
            "00112233-4455-6677-8899-AABBCCDDEEFF",
        ]
        let session = RemoteUploadSession(
            sessionID: "01234567-89AB-CDEF-0123-456789ABCDEF",
            identifierGenerator: { identifiers.removeFirst() }
        )
        let first = SSHDestination(host: "first.example.com")
        let second = SSHDestination(host: "second.example.com")

        let firstAttempt = session.begin(surfaceGeneration: 4, destination: first)
        let secondAttempt = session.begin(surfaceGeneration: 4, destination: second)
        let cleanup = try #require(session.takeActiveSessionForCleanup())

        #expect(firstAttempt.uploadID != secondAttempt.uploadID)
        #expect(cleanup.destinations == [first, second])
        #expect(!session.isActive)
    }

    @Test("cancellation generation and live surface all gate side effects")
    func sideEffectGates() {
        let session = RemoteUploadSession(
            sessionID: "01234567-89AB-CDEF-0123-456789ABCDEF",
            identifierGenerator: { "ABCDEF01-2345-6789-ABCD-EF0123456789" }
        )
        let attempt = session.begin(surfaceGeneration: 2, destination: SSHDestination(host: "example.com"))

        #expect(!session.permitsSideEffects(
            for: attempt,
            currentDestination: attempt.destination,
            surfaceGeneration: 2,
            hasLiveSurface: true,
            isCancelled: true
        ))
        #expect(!session.permitsSideEffects(
            for: attempt,
            currentDestination: attempt.destination,
            surfaceGeneration: 3,
            hasLiveSurface: true,
            isCancelled: false
        ))
        #expect(!session.permitsSideEffects(
            for: attempt,
            currentDestination: attempt.destination,
            surfaceGeneration: 2,
            hasLiveSurface: false,
            isCancelled: false
        ))
    }

    @Test("changing or leaving the SSH destination invalidates an attempt")
    func destinationChangeGatesSideEffects() {
        let session = RemoteUploadSession(
            sessionID: "01234567-89AB-CDEF-0123-456789ABCDEF",
            identifierGenerator: { "ABCDEF01-2345-6789-ABCD-EF0123456789" }
        )
        let original = SSHDestination(host: "first.example.com")
        let attempt = session.begin(surfaceGeneration: 2, destination: original)
        _ = session.begin(surfaceGeneration: 2, destination: SSHDestination(host: "second.example.com"))

        #expect(!session.permitsSideEffects(
            for: attempt,
            currentDestination: SSHDestination(host: "second.example.com"),
            surfaceGeneration: 2,
            hasLiveSurface: true,
            isCancelled: false
        ))
        #expect(!session.permitsSideEffects(
            for: attempt,
            currentDestination: nil,
            surfaceGeneration: 2,
            hasLiveSurface: true,
            isCancelled: false
        ))
        #expect(session.permitsSideEffects(
            for: attempt,
            currentDestination: original,
            surfaceGeneration: 2,
            hasLiveSurface: true,
            isCancelled: false
        ))
    }

    @Test("terminal attempts match only their captured route")
    func terminalAttemptRouteMatching() {
        let destination = SSHDestination(host: "example.com")
        let remote = TerminalUploadAttempt.remote(RemoteUploadAttempt(
            sessionID: "01234567-89AB-CDEF-0123-456789ABCDEF",
            uploadID: "ABCDEF01-2345-6789-ABCD-EF0123456789",
            surfaceGeneration: 0,
            destination: destination
        ))

        #expect(TerminalUploadAttempt.local(surfaceGeneration: 0).matches(destination: nil))
        #expect(!TerminalUploadAttempt.local(surfaceGeneration: 0).matches(destination: destination))
        #expect(remote.matches(destination: destination))
        #expect(!remote.matches(destination: nil))
    }
}
