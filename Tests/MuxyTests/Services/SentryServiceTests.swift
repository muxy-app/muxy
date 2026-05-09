import Foundation
import Testing

@testable import Muxy

@Suite("SentryService")
@MainActor
struct SentryServiceTests {
    @Test("needsPrompt is true when DSN is present and consent is undecided")
    func needsPromptWhenDSNAndUndecided() {
        let (service, _, suiteName) = makeService(dsn: "https://public@example.ingest.sentry.io/1")
        defer { UserDefaults().removePersistentDomain(forName: suiteName) }

        #expect(service.hasDSN)
        #expect(service.consent == nil)
        #expect(service.needsPrompt)
    }

    @Test("needsPrompt is false when DSN is missing")
    func needsPromptFalseWithoutDSN() {
        let (service, _, suiteName) = makeService(dsn: nil)
        defer { UserDefaults().removePersistentDomain(forName: suiteName) }

        #expect(!service.hasDSN)
        #expect(!service.needsPrompt)
    }

    @Test("setConsent persists denied and reports needsPrompt false")
    func setConsentDeniedPersists() {
        let (service, defaults, suiteName) = makeService(dsn: "https://public@example.ingest.sentry.io/1")
        defer { defaults.removePersistentDomain(forName: suiteName) }

        service.setConsent(.denied)

        #expect(service.consent == .denied)
        #expect(!service.needsPrompt)
        #expect(defaults.string(forKey: SentryConsent.storageKey) == "denied")
    }

    @Test("setConsent allowed starts the SDK; denied stops it")
    func setConsentTogglesStartAndStop() {
        var startCount = 0
        var stopCount = 0
        let (service, defaults, suiteName) = makeService(
            dsn: "https://public@example.ingest.sentry.io/1",
            starter: { _ in startCount += 1 },
            stopper: { stopCount += 1 }
        )
        defer { defaults.removePersistentDomain(forName: suiteName) }

        service.setConsent(.allowed)
        #expect(startCount == 1)
        #expect(stopCount == 0)

        service.setConsent(.allowed)
        #expect(startCount == 1, "start must be idempotent")

        service.setConsent(.denied)
        #expect(stopCount == 1)

        service.setConsent(.denied)
        #expect(stopCount == 1, "stop must be idempotent")
    }

    @Test("start is a no-op when DSN is missing even with allowed consent")
    func startNoOpWithoutDSN() {
        var startCount = 0
        let (service, defaults, suiteName) = makeService(
            dsn: nil,
            starter: { _ in startCount += 1 }
        )
        defer { defaults.removePersistentDomain(forName: suiteName) }

        service.setConsent(.allowed)

        #expect(startCount == 0)
    }

    @Test("loads previously stored consent on init")
    func loadsPersistedConsent() {
        let suiteName = "muxy.tests.sentry.\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            Issue.record("Unable to create isolated UserDefaults suite")
            return
        }
        defer { defaults.removePersistentDomain(forName: suiteName) }
        defaults.set(SentryConsent.allowed.rawValue, forKey: SentryConsent.storageKey)

        let service = SentryService(
            dsn: "https://public@example.ingest.sentry.io/1",
            defaults: defaults,
            starter: { _ in },
            stopper: {}
        )

        #expect(service.consent == .allowed)
        #expect(!service.needsPrompt)
    }

    @Test("environment is derived from the injected defaults' update channel")
    func startContextEnvironmentReflectsChannel() {
        var capturedEnvironments: [String] = []
        let (service, defaults, suiteName) = makeService(
            dsn: "https://public@example.ingest.sentry.io/1",
            starter: { context in capturedEnvironments.append(context.environment) }
        )
        defer { defaults.removePersistentDomain(forName: suiteName) }

        defaults.set(UpdateChannel.beta.rawValue, forKey: UpdateChannel.storageKey)
        service.setConsent(.allowed)

        #expect(capturedEnvironments == ["beta"])
    }

    private func makeService(
        dsn: String?,
        starter: @escaping (SentryStartContext) -> Void = { _ in },
        stopper: @escaping () -> Void = {}
    ) -> (SentryService, UserDefaults, String) {
        let suiteName = "muxy.tests.sentry.\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            fatalError("Unable to create isolated UserDefaults suite")
        }
        let service = SentryService(
            dsn: dsn,
            defaults: defaults,
            starter: starter,
            stopper: stopper
        )
        return (service, defaults, suiteName)
    }
}
