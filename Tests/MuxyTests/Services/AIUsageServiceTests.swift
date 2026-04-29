import Foundation
import Testing

@testable import Muxy

@Suite("AIUsageService")
struct AIUsageServiceTests {
    @Test("tracked provider defaults to false when unset and persists updates")
    func trackedProviderPersistence() {
        let suiteName = "AIUsageServiceTests.\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            Issue.record("Unable to create isolated UserDefaults suite")
            return
        }
        defer { defaults.removePersistentDomain(forName: suiteName) }

        let providerID = "claude"

        #expect(!AIUsageProviderTrackingStore.isTracked(providerID: providerID, defaults: defaults))
        #expect(!AIUsageProviderTrackingStore.hasTrackedPreference(providerID: providerID, defaults: defaults))

        AIUsageProviderTrackingStore.setTracked(false, providerID: providerID, defaults: defaults)
        #expect(!AIUsageProviderTrackingStore.isTracked(providerID: providerID, defaults: defaults))
        #expect(AIUsageProviderTrackingStore.hasTrackedPreference(providerID: providerID, defaults: defaults))

        AIUsageProviderTrackingStore.setTracked(true, providerID: providerID, defaults: defaults)
        #expect(AIUsageProviderTrackingStore.isTracked(providerID: providerID, defaults: defaults))
        #expect(AIUsageProviderTrackingStore.hasTrackedPreference(providerID: providerID, defaults: defaults))
    }

    @Test("auto-track enables providers with available usage when no explicit tracking preference exists")
    func autoTrackAvailableUsageWhenUnset() {
        let suiteName = "AIUsageServiceTests.AutoTrackAvailable.\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            Issue.record("Unable to create isolated UserDefaults suite")
            return
        }
        defer { defaults.removePersistentDomain(forName: suiteName) }

        let providerID = "codex"
        #expect(!AIUsageProviderTrackingStore.hasTrackedPreference(providerID: providerID, defaults: defaults))

        let snapshots = [
            AIProviderUsageSnapshot(
                providerID: providerID,
                providerName: "Codex",
                providerIconName: "sparkles",
                state: .available,
                rows: [AIUsageMetricRow(label: "Monthly", percent: 45, resetDate: nil, detail: "45/100")]
            ),
        ]

        AIUsageAutoTracking.autoTrackProvidersWithAvailableUsage(snapshots: snapshots, defaults: defaults)

        #expect(AIUsageProviderTrackingStore.hasTrackedPreference(providerID: providerID, defaults: defaults))
        #expect(AIUsageProviderTrackingStore.isTracked(providerID: providerID, defaults: defaults))
    }

    @Test("explicit false tracking preference is not overridden by auto-track")
    func autoTrackDoesNotOverrideExplicitFalse() {
        let suiteName = "AIUsageServiceTests.AutoTrackExplicitFalse.\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            Issue.record("Unable to create isolated UserDefaults suite")
            return
        }
        defer { defaults.removePersistentDomain(forName: suiteName) }

        let providerID = "codex"
        AIUsageProviderTrackingStore.setTracked(false, providerID: providerID, defaults: defaults)

        let snapshots = [
            AIProviderUsageSnapshot(
                providerID: providerID,
                providerName: "Codex",
                providerIconName: "sparkles",
                state: .available,
                rows: [AIUsageMetricRow(label: "Monthly", percent: 80, resetDate: nil, detail: "80/100")]
            ),
        ]

        AIUsageAutoTracking.autoTrackProvidersWithAvailableUsage(snapshots: snapshots, defaults: defaults)

        #expect(AIUsageProviderTrackingStore.hasTrackedPreference(providerID: providerID, defaults: defaults))
        #expect(!AIUsageProviderTrackingStore.isTracked(providerID: providerID, defaults: defaults))
    }

    @Test("provider enabled defaults to true and persists updates")
    func providerEnabledPersistence() {
        let suiteName = "AIUsageServiceTests.ProviderEnabled.\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            Issue.record("Unable to create isolated UserDefaults suite")
            return
        }
        defer { defaults.removePersistentDomain(forName: suiteName) }

        let providerID = "codex"

        #expect(AIUsageProviderEnabledStore.isEnabled(providerID: providerID, defaults: defaults))

        AIUsageProviderEnabledStore.setEnabled(false, providerID: providerID, defaults: defaults)
        #expect(!AIUsageProviderEnabledStore.isEnabled(providerID: providerID, defaults: defaults))

        AIUsageProviderEnabledStore.setEnabled(true, providerID: providerID, defaults: defaults)
        #expect(AIUsageProviderEnabledStore.isEnabled(providerID: providerID, defaults: defaults))
    }

    @Test("auto refresh interval defaults to 5 minutes and persists updates")
    func autoRefreshIntervalPersistence() {
        let suiteName = "AIUsageServiceTests.AutoRefreshInterval.\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            Issue.record("Unable to create isolated UserDefaults suite")
            return
        }
        defer { defaults.removePersistentDomain(forName: suiteName) }

        #expect(AIUsageSettingsStore.autoRefreshInterval(defaults: defaults) == .fiveMinutes)

        AIUsageSettingsStore.setAutoRefreshInterval(.fifteenMinutes, defaults: defaults)
        #expect(AIUsageSettingsStore.autoRefreshInterval(defaults: defaults) == .fifteenMinutes)

        AIUsageSettingsStore.setAutoRefreshInterval(.thirtyMinutes, defaults: defaults)
        #expect(AIUsageSettingsStore.autoRefreshInterval(defaults: defaults) == .thirtyMinutes)

        AIUsageSettingsStore.setAutoRefreshInterval(.oneHour, defaults: defaults)
        #expect(AIUsageSettingsStore.autoRefreshInterval(defaults: defaults) == .oneHour)
    }

    @Test("auto refresh interval has expected options labels and raw values")
    func autoRefreshIntervalOptions() {
        #expect(AIUsageAutoRefreshInterval.allCases == [.fiveMinutes, .fifteenMinutes, .thirtyMinutes, .oneHour])
        #expect(AIUsageAutoRefreshInterval.fiveMinutes.rawValue == 300)
        #expect(AIUsageAutoRefreshInterval.fifteenMinutes.rawValue == 900)
        #expect(AIUsageAutoRefreshInterval.thirtyMinutes.rawValue == 1800)
        #expect(AIUsageAutoRefreshInterval.oneHour.rawValue == 3600)

        #expect(AIUsageAutoRefreshInterval.fiveMinutes.label == "5 min")
        #expect(AIUsageAutoRefreshInterval.fifteenMinutes.label == "15 min")
        #expect(AIUsageAutoRefreshInterval.thirtyMinutes.label == "30 min")
        #expect(AIUsageAutoRefreshInterval.oneHour.label == "1h")
    }

    @Test("tracking preferences canonicalize legacy provider IDs")
    func trackingCanonicalizesLegacyProviderIDs() {
        let suiteName = "AIUsageServiceTests.CanonicalTracking.\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            Issue.record("Unable to create isolated UserDefaults suite")
            return
        }
        defer { defaults.removePersistentDomain(forName: suiteName) }

        AIUsageProviderTrackingStore.setTracked(true, providerID: "claude_code", defaults: defaults)

        #expect(AIUsageProviderTrackingStore.isTracked(providerID: "claude", defaults: defaults))
        #expect(AIUsageProviderTrackingStore.hasTrackedPreference(providerID: "claude", defaults: defaults))
    }

    @MainActor
    @Test("catalog canonical ID normalizes legacy provider IDs")
    func catalogCanonicalizesLegacyProviderIDs() {
        #expect(AIUsageProviderCatalog.canonicalID(for: "claude_code") == "claude")
        #expect(AIUsageProviderCatalog.canonicalID(for: "codex") == "codex")
    }

    @Test("compose snapshots includes tracked disabled providers with Disabled state")
    func composeSnapshotsIncludesDisabledProviderState() {
        let trackedProviders = [
            AITrackedProviderUsageDescriptor(
                providerID: "codex",
                providerName: "Codex",
                providerIconName: "sparkles",
                isEnabled: false
            ),
            AITrackedProviderUsageDescriptor(
                providerID: "claude",
                providerName: "Claude Code",
                providerIconName: "sparkles",
                isEnabled: true
            ),
        ]

        let fetchedSnapshots = [
            AIProviderUsageSnapshot(
                providerID: "claude",
                providerName: "Claude Code",
                providerIconName: "sparkles",
                state: .available,
                rows: []
            ),
        ]

        let composed = AIUsageSnapshotComposer.compose(
            trackedProviders: trackedProviders,
            fetchedSnapshots: fetchedSnapshots
        )

        #expect(composed.count == 2)
        #expect(composed[0].providerID == "codex")
        #expect(composed[1].providerID == "claude")

        if case let .unavailable(message) = composed[0].state {
            #expect(message == "No usage data")
        } else {
            Issue.record("Expected disabled provider to map to unavailable state")
        }
    }

    @MainActor
    @Test("bundled provider catalog metadata is exposed for usage-only providers")
    func bundledProviderCatalogMetadata() {
        let entry = AIUsageProviderCatalog.entry(providerID: "copilot")
        #expect(entry?.displayName == "Copilot")
        #expect(entry?.hasNotificationIntegration == false)
        #expect(entry?.isBundled == true)
    }
}
