import Foundation
import os

let usageLogger = Logger(subsystem: "app.muxy", category: "AIUsageService")

enum AIUsageProviderCatalogSource {
    case notificationIntegration
    case bundled
}

struct AIUsageProviderCatalogEntry: Identifiable, Equatable {
    let id: String
    let displayName: String
    let iconName: String
    let source: AIUsageProviderCatalogSource

    var hasNotificationIntegration: Bool { source == .notificationIntegration }
    var isBundled: Bool { source == .bundled }
}

@MainActor
enum AIUsageProviderCatalog {
    static let providers: [AIUsageProviderCatalogEntry] = {
        let notificationIDs = Set(AIProviderRegistry.shared.providers.map { canonicalAIUsageProviderID($0.id) })

        return AIProviderRegistry.shared.usageProviders.map { provider in
            let canonicalID = canonicalAIUsageProviderID(provider.id)
            return AIUsageProviderCatalogEntry(
                id: canonicalID,
                displayName: provider.displayName,
                iconName: provider.iconName,
                source: notificationIDs.contains(canonicalID) ? .notificationIntegration : .bundled
            )
        }
        .sorted { lhs, rhs in
            let lhsIntegrated = lhs.hasNotificationIntegration
            let rhsIntegrated = rhs.hasNotificationIntegration
            if lhsIntegrated != rhsIntegrated {
                return lhsIntegrated
            }
            return lhs.displayName.localizedCaseInsensitiveCompare(rhs.displayName) == .orderedAscending
        }
    }()

    private static let providerByID: [String: AIUsageProviderCatalogEntry] =
        Dictionary(uniqueKeysWithValues: providers.map { ($0.id, $0) })

    private static let notificationProviderByID: [String: any AIProviderIntegration] =
        Dictionary(uniqueKeysWithValues: AIProviderRegistry.shared.providers.map { (canonicalAIUsageProviderID($0.id), $0) })

    static func entry(providerID: String) -> AIUsageProviderCatalogEntry? {
        providerByID[canonicalAIUsageProviderID(providerID)]
    }

    static func notificationProvider(providerID: String) -> (any AIProviderIntegration)? {
        notificationProviderByID[canonicalAIUsageProviderID(providerID)]
    }

    static func canonicalID(for providerID: String) -> String {
        canonicalAIUsageProviderID(providerID)
    }
}

enum AIUsageSnapshotComposer {
    static func compose(
        trackedProviders: [AITrackedProviderUsageDescriptor],
        fetchedSnapshots: [AIProviderUsageSnapshot],
        includeSecondary: Bool = false
    ) -> [AIProviderUsageSnapshot] {
        let snapshotByProviderID = Dictionary(uniqueKeysWithValues: fetchedSnapshots
            .map { (canonicalAIUsageProviderID($0.providerID), $0) })

        return trackedProviders.map { provider in
            if !provider.isEnabled {
                return AIProviderUsageSnapshot(
                    providerID: provider.providerID,
                    providerName: provider.providerName,
                    providerIconName: provider.providerIconName,
                    state: .unavailable(message: "No usage data"),
                    rows: []
                )
            }

            if let snapshot = snapshotByProviderID[canonicalAIUsageProviderID(provider.providerID)] {
                return filterVisibleRows(snapshot, includeSecondary: includeSecondary)
            }

            return AIProviderUsageSnapshot(
                providerID: provider.providerID,
                providerName: provider.providerName,
                providerIconName: provider.providerIconName,
                state: .unavailable(message: "No usage data"),
                rows: []
            )
        }
    }

    private static func filterVisibleRows(
        _ snapshot: AIProviderUsageSnapshot,
        includeSecondary: Bool
    ) -> AIProviderUsageSnapshot {
        guard case .available = snapshot.state else { return snapshot }

        let visibleRows = snapshot.rows.filter { row in
            AIUsageRowPolicy.isVisible(row, includeSecondary: includeSecondary)
        }
        if visibleRows.isEmpty {
            return AIProviderUsageSnapshot(
                providerID: snapshot.providerID,
                providerName: snapshot.providerName,
                providerIconName: snapshot.providerIconName,
                fetchedAt: snapshot.fetchedAt,
                state: .unavailable(message: "No usage data"),
                rows: []
            )
        }

        return AIProviderUsageSnapshot(
            providerID: snapshot.providerID,
            providerName: snapshot.providerName,
            providerIconName: snapshot.providerIconName,
            fetchedAt: snapshot.fetchedAt,
            state: snapshot.state,
            rows: visibleRows
        )
    }
}

enum AIUsageRowPolicy {
    private static let primaryLabelPrefixes = ["session", "5h", "premium", "hourly", "primary"]
    private static let secondaryLabelPrefixes = ["weekly", "week", "7d", "monthly", "month", "daily", "day", "billing"]

    static func isPrimary(_ row: AIUsageMetricRow) -> Bool {
        matches(row, prefixes: primaryLabelPrefixes)
    }

    static func isSecondary(_ row: AIUsageMetricRow) -> Bool {
        matches(row, prefixes: secondaryLabelPrefixes)
    }

    static func isVisible(_ row: AIUsageMetricRow, includeSecondary: Bool) -> Bool {
        if isPrimary(row) { return true }
        if includeSecondary, isSecondary(row) { return true }
        return false
    }

    private static func matches(_ row: AIUsageMetricRow, prefixes: [String]) -> Bool {
        if let detail = row.detail, detail.contains("$") { return false }
        let label = row.label.trimmingCharacters(in: .whitespaces).lowercased()
        return prefixes.contains { label.hasPrefix($0) }
    }
}

@MainActor
@Observable
final class AIUsageService {
    static let shared = AIUsageService()

    private(set) var snapshots: [AIProviderUsageSnapshot] = []
    private(set) var isRefreshing = false
    private(set) var lastRefreshDate: Date?

    var minimumRefreshInterval: TimeInterval {
        AIUsageSettingsStore.autoRefreshInterval().timeInterval
    }

    @ObservationIgnored private var refreshTask: Task<[AIProviderUsageSnapshot], Never>?
    @ObservationIgnored private var fetchedSnapshotsCache: [AIProviderUsageSnapshot] = []
    @ObservationIgnored private var previousSnapshotsCache: [AIProviderUsageSnapshot] = []

    private func usedPercent(for snapshot: AIProviderUsageSnapshot) -> Double? {
        guard case .available = snapshot.state else { return nil }
        guard let maxPercent = snapshot.rows.compactMap(\.percent).max() else { return nil }
        return max(0, min(100, maxPercent))
    }

    var mostUsedProviderSnapshot: AIProviderUsageSnapshot? {
        snapshots
            .filter { snapshot in
                guard case .available = snapshot.state else { return false }
                return snapshot.rows.contains { $0.percent != nil }
            }
            .max {
                ($0.rows.compactMap(\.percent).max() ?? 0) < ($1.rows.compactMap(\.percent).max() ?? 0)
            }
    }

    var previewProviderSnapshot: AIProviderUsageSnapshot? {
        previewProviderSnapshot(pinnedRawValue: UserDefaults.standard
            .string(forKey: AIUsageSettingsStore.sidebarPreviewProviderIDKey) ?? "")
    }

    func previewProviderSnapshot(pinnedRawValue: String) -> AIProviderUsageSnapshot? {
        if let pinned = snapshots.first(where: { isPinnedCandidate($0, pinnedRawValue: pinnedRawValue) }) {
            return pinned
        }
        return mostActiveProviderSnapshot ?? mostUsedProviderSnapshot
    }

    private func isPinnedCandidate(_ snapshot: AIProviderUsageSnapshot, pinnedRawValue: String) -> Bool {
        guard AIUsageSettingsStore.isSidebarPinned(providerID: snapshot.providerID, pinnedRawValue: pinnedRawValue) else {
            return false
        }
        guard case .available = snapshot.state else { return false }
        return snapshot.rows.contains { $0.percent != nil }
    }

    var mostActiveProviderSnapshot: AIProviderUsageSnapshot? {
        guard !snapshots.isEmpty, !previousSnapshotsCache.isEmpty else { return nil }

        var maxDelta: Double = 0
        var mostActive: AIProviderUsageSnapshot?

        for current in snapshots {
            guard let currentPercent = usedPercent(for: current),
                  let previous = previousSnapshotsCache.first(where: { $0.providerID == current.providerID }),
                  let previousPercent = usedPercent(for: previous)
            else { continue }

            let delta = abs(currentPercent - previousPercent)
            if delta > maxDelta {
                maxDelta = delta
                mostActive = current
            }
        }

        return mostActive
    }

    private init() {}

    private struct ProviderRuntimePreferences {
        let trackedProviderIDs: Set<String>
        let enabledByProviderID: [String: Bool]

        @MainActor
        init(catalogProviders: [AIUsageProviderCatalogEntry], defaults: UserDefaults = .standard) {
            var trackedProviderIDs: Set<String> = []
            trackedProviderIDs.reserveCapacity(catalogProviders.count)

            var enabledByProviderID: [String: Bool] = [:]
            enabledByProviderID.reserveCapacity(catalogProviders.count)

            for provider in catalogProviders {
                let providerID = provider.id

                if AIUsageProviderTrackingStore.trackedPreference(providerID: providerID, defaults: defaults) == true {
                    trackedProviderIDs.insert(providerID)
                }

                enabledByProviderID[providerID] = AIUsageProviderEnabledStore.isEnabled(providerID: providerID, defaults: defaults)
            }

            self.trackedProviderIDs = trackedProviderIDs
            self.enabledByProviderID = enabledByProviderID
        }
    }

    func refreshIfNeeded(force: Bool = false) async {
        guard AIUsageSettingsStore.isUsageEnabled() else { return }

        if let refreshTask {
            _ = await refreshTask.value
            return
        }

        guard force || shouldRefresh(at: Date()) else { return }

        await refresh(force: true)
    }

    func refresh(force: Bool = false) async {
        guard AIUsageSettingsStore.isUsageEnabled() else { return }

        if let refreshTask {
            _ = await refreshTask.value
            return
        }

        if !force, !shouldRefresh(at: Date()) {
            return
        }

        let catalogProviders = AIUsageProviderCatalog.providers
        let preferences = ProviderRuntimePreferences(catalogProviders: catalogProviders)
        let enabledProviders = AIProviderRegistry.shared.usageProviders.filter { provider in
            preferences.enabledByProviderID[canonicalAIUsageProviderID(provider.id)] == true
        }

        isRefreshing = true
        defer {
            isRefreshing = false
            refreshTask = nil
        }

        let task = Task<[AIProviderUsageSnapshot], Never>.detached(priority: .userInitiated) {
            await AIUsageService.fetchSnapshots(for: enabledProviders)
        }

        refreshTask = task
        let fetchedSnapshots = await task.value
        AIUsageAutoTracking.autoTrackProvidersWithAvailableUsage(snapshots: fetchedSnapshots)

        previousSnapshotsCache = snapshots
        fetchedSnapshotsCache = fetchedSnapshots
        let composedSnapshots = composeSnapshots(
            catalogProviders: catalogProviders,
            fetchedSnapshots: fetchedSnapshots
        )

        if snapshots != composedSnapshots {
            snapshots = composedSnapshots
        }
        lastRefreshDate = Date()
    }

    private static func fetchSnapshots(for providers: [any AIUsageProvider]) async -> [AIProviderUsageSnapshot] {
        await withTaskGroup(of: (Int, AIProviderUsageSnapshot).self) { group in
            for (index, provider) in providers.enumerated() {
                group.addTask {
                    await (index, provider.fetchUsageSnapshot())
                }
            }

            var indexed: [(Int, AIProviderUsageSnapshot)] = []
            indexed.reserveCapacity(providers.count)
            for await pair in group {
                indexed.append(pair)
            }
            return indexed.sorted { $0.0 < $1.0 }.map(\.1)
        }
    }

    private func shouldRefresh(at date: Date) -> Bool {
        guard let lastRefreshDate else { return true }

        let interval = AIUsageSettingsStore.autoRefreshInterval()
        return date.timeIntervalSince(lastRefreshDate) >= interval.timeInterval
    }

    func recomposeSnapshots() {
        let catalogProviders = AIUsageProviderCatalog.providers
        let recomposed = composeSnapshots(
            catalogProviders: catalogProviders,
            fetchedSnapshots: fetchedSnapshotsCache
        )
        if snapshots != recomposed {
            snapshots = recomposed
        }
    }

    private func composeSnapshots(
        catalogProviders: [AIUsageProviderCatalogEntry],
        fetchedSnapshots: [AIProviderUsageSnapshot]
    ) -> [AIProviderUsageSnapshot] {
        let updatedPreferences = ProviderRuntimePreferences(catalogProviders: catalogProviders)
        let trackedProviders = catalogProviders.compactMap { provider -> AITrackedProviderUsageDescriptor? in
            guard updatedPreferences.trackedProviderIDs.contains(provider.id) else { return nil }
            return AITrackedProviderUsageDescriptor(
                providerID: provider.id,
                providerName: provider.displayName,
                providerIconName: provider.iconName,
                isEnabled: updatedPreferences.enabledByProviderID[provider.id] ?? true
            )
        }

        return AIUsageSnapshotComposer.compose(
            trackedProviders: trackedProviders,
            fetchedSnapshots: fetchedSnapshots,
            includeSecondary: AIUsageSettingsStore.showSecondaryLimits()
        )
    }
}
