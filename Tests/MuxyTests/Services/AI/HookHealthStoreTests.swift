import Foundation
import Testing

@testable import Muxy

@Suite("HookHealthStore")
@MainActor
struct HookHealthStoreTests {
    @Test("noteVerified records install state and verification time")
    func noteVerifiedRecordsState() {
        let clock = Clock()
        let store = HookHealthStore(now: clock.now)
        store.noteVerified(providerID: "claude", state: .installed)

        let health = store.health(for: "claude")
        #expect(health.installState == .installed)
        #expect(health.lastVerifiedAt == clock.value)
        #expect(health.lastError == nil)
        #expect(health.lastRepairedAt == nil)
    }

    @Test("noteVerified stores error message for conflict and error states")
    func noteVerifiedStoresErrorMessage() {
        let store = HookHealthStore(now: { Date() })
        store.noteVerified(providerID: "codex", state: .conflict("inline hooks"))
        #expect(store.health(for: "codex").lastError == "inline hooks")

        store.noteVerified(providerID: "codex", state: .error("boom"))
        #expect(store.health(for: "codex").lastError == "boom")

        store.noteVerified(providerID: "codex", state: .installed)
        #expect(store.health(for: "codex").lastError == nil)
    }

    @Test("noteRepaired records both verified and repaired times")
    func noteRepairedRecordsTimes() {
        let clock = Clock()
        let store = HookHealthStore(now: clock.now)
        store.noteRepaired(providerID: "grok", state: .installed)

        let health = store.health(for: "grok")
        #expect(health.installState == .installed)
        #expect(health.lastVerifiedAt == clock.value)
        #expect(health.lastRepairedAt == clock.value)
    }

    @Test("noteEvent stamps lastEventAt without altering install state")
    func noteEventStampsLastEventAt() {
        let clock = Clock()
        let store = HookHealthStore(now: clock.now)
        store.noteVerified(providerID: "claude", state: .installed)
        store.noteEvent(providerID: "claude")

        let health = store.health(for: "claude")
        #expect(health.lastEventAt == clock.value)
        #expect(health.installState == .installed)
    }

    @Test("noteEvent creates an entry for a previously unknown provider")
    func noteEventCreatesEntry() {
        let store = HookHealthStore(now: { Date() })
        store.noteEvent(providerID: "pi")
        #expect(store.health(for: "pi").lastEventAt != nil)
    }

    @Test("reset clears provider health")
    func resetClearsHealth() {
        let store = HookHealthStore(now: { Date() })
        store.noteVerified(providerID: "droid", state: .installed)
        store.noteEvent(providerID: "droid")
        store.reset(providerID: "droid")

        #expect(store.health(for: "droid") == HookHealth())
    }

    private final class Clock: @unchecked Sendable {
        let value = Date(timeIntervalSince1970: 1_700_000_000)
        func now() -> Date { value }
    }
}
