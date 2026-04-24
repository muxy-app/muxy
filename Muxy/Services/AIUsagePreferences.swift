import Foundation

func canonicalAIUsageProviderID(_ providerID: String) -> String {
    let normalized = providerID.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
    switch normalized {
    case "claude_code":
        return "claude"
    default:
        return normalized
    }
}

struct AITrackedProviderUsageDescriptor: Equatable {
    let providerID: String
    let providerName: String
    let providerIconName: String
    let isEnabled: Bool
}

enum AIUsageProviderTrackingStore {
    static func trackingKey(providerID: String) -> String {
        "muxy.usage.provider.\(canonicalAIUsageProviderID(providerID)).tracked"
    }

    static func trackedPreference(providerID: String, defaults: UserDefaults = .standard) -> Bool? {
        let key = trackingKey(providerID: providerID)
        guard defaults.object(forKey: key) != nil else { return nil }
        return defaults.bool(forKey: key)
    }

    static func hasTrackedPreference(providerID: String, defaults: UserDefaults = .standard) -> Bool {
        trackedPreference(providerID: providerID, defaults: defaults) != nil
    }

    static func isTracked(providerID: String, defaults: UserDefaults = .standard) -> Bool {
        trackedPreference(providerID: providerID, defaults: defaults) ?? false
    }

    static func setTracked(_ tracked: Bool, providerID: String, defaults: UserDefaults = .standard) {
        defaults.set(tracked, forKey: trackingKey(providerID: providerID))
    }
}

enum AIUsageAutoTracking {
    static func autoTrackProvidersWithAvailableUsage(
        snapshots: [AIProviderUsageSnapshot],
        defaults: UserDefaults = .standard
    ) {
        for snapshot in snapshots where hasAvailableUsage(snapshot) {
            if !AIUsageProviderTrackingStore.hasTrackedPreference(providerID: snapshot.providerID, defaults: defaults) {
                AIUsageProviderTrackingStore.setTracked(true, providerID: snapshot.providerID, defaults: defaults)
            }
        }
    }

    private static func hasAvailableUsage(_ snapshot: AIProviderUsageSnapshot) -> Bool {
        guard case .available = snapshot.state else { return false }
        return !snapshot.rows.isEmpty
    }
}

enum AIUsageProviderEnabledStore {
    static func enabledKey(providerID: String) -> String {
        "muxy.usage.provider.\(canonicalAIUsageProviderID(providerID)).enabled"
    }

    static func isEnabled(providerID: String, defaults: UserDefaults = .standard) -> Bool {
        defaults.bool(forKey: enabledKey(providerID: providerID), fallback: true)
    }

    static func setEnabled(_ enabled: Bool, providerID: String, defaults: UserDefaults = .standard) {
        defaults.set(enabled, forKey: enabledKey(providerID: providerID))
    }
}

enum AIUsageDisplayMode: String, CaseIterable, Identifiable {
    case used
    case remaining

    var id: String { rawValue }

    var label: String {
        switch self {
        case .used:
            "Used"
        case .remaining:
            "Remaining"
        }
    }
}

enum AIUsageSettingsStore {
    static let autoRefreshIntervalKey = "muxy.usage.autoRefreshIntervalSeconds"
    static let usageDisplayModeKey = "muxy.usage.displayMode"
    static let usageEnabledKey = "muxy.usage.enabled"
    static let showSecondaryLimitsKey = "muxy.usage.showSecondaryLimits"
    static let sidebarPreviewProviderIDKey = "muxy.usage.sidebarPreviewProviderID"

    static let defaultAutoRefreshInterval: AIUsageAutoRefreshInterval = .fiveMinutes
    static let defaultUsageDisplayMode: AIUsageDisplayMode = .used
    static let defaultShowSecondaryLimits = false

    static func autoRefreshInterval(defaults: UserDefaults = .standard) -> AIUsageAutoRefreshInterval {
        guard defaults.object(forKey: autoRefreshIntervalKey) != nil else {
            return defaultAutoRefreshInterval
        }

        let rawValue = defaults.integer(forKey: autoRefreshIntervalKey)
        return AIUsageAutoRefreshInterval(rawValue: rawValue) ?? defaultAutoRefreshInterval
    }

    static func setAutoRefreshInterval(_ interval: AIUsageAutoRefreshInterval, defaults: UserDefaults = .standard) {
        defaults.set(interval.rawValue, forKey: autoRefreshIntervalKey)
    }

    static func usageDisplayMode(defaults: UserDefaults = .standard) -> AIUsageDisplayMode {
        guard let raw = defaults.string(forKey: usageDisplayModeKey),
              let mode = AIUsageDisplayMode(rawValue: raw)
        else {
            return defaultUsageDisplayMode
        }
        return mode
    }

    static func setUsageDisplayMode(_ mode: AIUsageDisplayMode, defaults: UserDefaults = .standard) {
        defaults.set(mode.rawValue, forKey: usageDisplayModeKey)
    }

    @MainActor
    static func isUsageEnabled(defaults: UserDefaults = .standard) -> Bool {
        if defaults.object(forKey: usageEnabledKey) != nil {
            return defaults.bool(forKey: usageEnabledKey)
        }

        var enabled = false
        for provider in AIUsageProviderCatalog.providers where AIUsageProviderTrackingStore.trackedPreference(
            providerID: provider.id,
            defaults: defaults
        ) == true {
            enabled = true
            break
        }

        defaults.set(enabled, forKey: usageEnabledKey)
        return enabled
    }

    static func setUsageEnabled(_ enabled: Bool, defaults: UserDefaults = .standard) {
        defaults.set(enabled, forKey: usageEnabledKey)
    }

    static func showSecondaryLimits(defaults: UserDefaults = .standard) -> Bool {
        guard defaults.object(forKey: showSecondaryLimitsKey) != nil else {
            return defaultShowSecondaryLimits
        }
        return defaults.bool(forKey: showSecondaryLimitsKey)
    }

    static func setShowSecondaryLimits(_ show: Bool, defaults: UserDefaults = .standard) {
        defaults.set(show, forKey: showSecondaryLimitsKey)
    }

    static func sidebarPreviewProviderID(defaults: UserDefaults = .standard) -> String? {
        guard let raw = defaults.string(forKey: sidebarPreviewProviderIDKey),
              !raw.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        else {
            return nil
        }
        return canonicalAIUsageProviderID(raw)
    }

    static func setSidebarPreviewProviderID(_ providerID: String?, defaults: UserDefaults = .standard) {
        if let providerID {
            defaults.set(canonicalAIUsageProviderID(providerID), forKey: sidebarPreviewProviderIDKey)
        } else {
            defaults.removeObject(forKey: sidebarPreviewProviderIDKey)
        }
    }

    static func isSidebarPinned(providerID: String, pinnedRawValue: String) -> Bool {
        let trimmed = pinnedRawValue.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return false }
        return canonicalAIUsageProviderID(trimmed) == canonicalAIUsageProviderID(providerID)
    }
}

enum AIUsageAutoRefreshInterval: Int, CaseIterable, Identifiable {
    case fiveMinutes = 300
    case fifteenMinutes = 900
    case thirtyMinutes = 1800
    case oneHour = 3600

    var id: Int { rawValue }

    var label: String {
        switch self {
        case .fiveMinutes:
            "5 min"
        case .fifteenMinutes:
            "15 min"
        case .thirtyMinutes:
            "30 min"
        case .oneHour:
            "1h"
        }
    }

    var timeInterval: TimeInterval {
        TimeInterval(rawValue)
    }
}
