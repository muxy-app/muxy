import Foundation
import Testing

@testable import Muxy

@Suite("VCSStateStore")
@MainActor
struct VCSStateStoreTests {
    @Test("Returns the same instance for the same path")
    func returnsSameInstanceForSamePath() {
        let store = VCSStateStore.shared
        let path = NSTemporaryDirectory() + "muxy-vcs-store-\(UUID().uuidString)"
        defer { store.remove(path: path) }

        let first = store.state(for: path)
        let second = store.state(for: path)

        #expect(first === second)
    }

    @Test("Canonicalizes paths so equivalent inputs share a state")
    func canonicalizesEquivalentPaths() {
        let store = VCSStateStore.shared
        let unique = "muxy-vcs-store-\(UUID().uuidString)"
        let pathWithSlash = NSTemporaryDirectory() + unique + "/"
        let pathPlain = NSTemporaryDirectory() + unique
        defer { store.remove(path: pathPlain) }

        let first = store.state(for: pathWithSlash)
        let second = store.state(for: pathPlain)

        #expect(first === second)
    }

    @Test("remove evicts the cached state")
    func removeEvictsCachedState() {
        let store = VCSStateStore.shared
        let path = NSTemporaryDirectory() + "muxy-vcs-store-\(UUID().uuidString)"

        let first = store.state(for: path)
        store.remove(path: path)
        let second = store.state(for: path)
        defer { store.remove(path: path) }

        #expect(first !== second)
    }

    @Test("cachedState returns nil before first state(for:) call")
    func cachedStateReturnsNilBeforeFirstAccess() {
        let store = VCSStateStore.shared
        let path = NSTemporaryDirectory() + "muxy-vcs-store-\(UUID().uuidString)"
        defer { store.remove(path: path) }

        #expect(store.cachedState(for: path) == nil)
        _ = store.state(for: path)
        #expect(store.cachedState(for: path) != nil)
    }
}
