import CoreGraphics
import CryptoKit
import Foundation

enum VCSPersistedSettings {
    private static let visibilityPrefix = "vcs.sectionVisibility."
    private static let autoSyncPrefix = "vcs.prAutoSyncMinutes."
    private static let collapsePrefix = "vcs.sectionCollapsed."
    private static let ratiosPrefix = "vcs.sectionRatios."

    struct SectionCollapse: Equatable {
        var staged: Bool
        var changes: Bool
        var history: Bool
        var pullRequests: Bool

        static let defaults = SectionCollapse(staged: false, changes: false, history: false, pullRequests: true)
    }

    struct SectionVisibility: Equatable {
        var changes: Bool
        var history: Bool
        var pullRequests: Bool

        static let defaults = SectionVisibility(changes: true, history: true, pullRequests: true)
    }

    static func loadSectionVisibility(repoPath: String) -> SectionVisibility {
        let defaults = UserDefaults.standard
        let raw = defaults.dictionary(forKey: visibilityPrefix + token(for: repoPath))
            ?? migrateLegacy(prefix: visibilityPrefix, repoPath: repoPath)
        guard let dict = raw as? [String: Bool] else { return .defaults }
        return SectionVisibility(
            changes: dict["changes"] ?? true,
            history: dict["history"] ?? true,
            pullRequests: dict["pullRequests"] ?? true
        )
    }

    static func storeSectionVisibility(_ visibility: SectionVisibility, repoPath: String) {
        let raw: [String: Bool] = [
            "changes": visibility.changes,
            "history": visibility.history,
            "pullRequests": visibility.pullRequests,
        ]
        UserDefaults.standard.set(raw, forKey: visibilityPrefix + token(for: repoPath))
    }

    static func loadAutoSyncMinutes(repoPath: String) -> Int {
        let defaults = UserDefaults.standard
        let key = autoSyncPrefix + token(for: repoPath)
        if defaults.object(forKey: key) != nil {
            return defaults.integer(forKey: key)
        }
        let legacyKey = autoSyncPrefix + repoPath
        if defaults.object(forKey: legacyKey) != nil {
            let value = defaults.integer(forKey: legacyKey)
            defaults.set(value, forKey: key)
            defaults.removeObject(forKey: legacyKey)
            return value
        }
        return 0
    }

    static func storeAutoSyncMinutes(_ minutes: Int, repoPath: String) {
        UserDefaults.standard.set(minutes, forKey: autoSyncPrefix + token(for: repoPath))
    }

    static func clearSettings(repoPath: String) {
        let defaults = UserDefaults.standard
        let token = token(for: repoPath)
        defaults.removeObject(forKey: visibilityPrefix + token)
        defaults.removeObject(forKey: autoSyncPrefix + token)
        defaults.removeObject(forKey: collapsePrefix + token)
        defaults.removeObject(forKey: ratiosPrefix + token)
        defaults.removeObject(forKey: visibilityPrefix + repoPath)
        defaults.removeObject(forKey: autoSyncPrefix + repoPath)
    }

    static func loadSectionCollapse(repoPath: String) -> SectionCollapse {
        let defaults = UserDefaults.standard
        guard let dict = defaults.dictionary(forKey: collapsePrefix + token(for: repoPath)) as? [String: Bool] else {
            return .defaults
        }
        return SectionCollapse(
            staged: dict["staged"] ?? false,
            changes: dict["changes"] ?? false,
            history: dict["history"] ?? false,
            pullRequests: dict["pullRequests"] ?? true
        )
    }

    static func storeSectionCollapse(_ collapse: SectionCollapse, repoPath: String) {
        let raw: [String: Bool] = [
            "staged": collapse.staged,
            "changes": collapse.changes,
            "history": collapse.history,
            "pullRequests": collapse.pullRequests,
        ]
        UserDefaults.standard.set(raw, forKey: collapsePrefix + token(for: repoPath))
    }

    static func loadSectionRatios(repoPath: String) -> [CGFloat] {
        let defaults = UserDefaults.standard
        guard let raw = defaults.array(forKey: ratiosPrefix + token(for: repoPath)) as? [Double] else {
            return []
        }
        return raw.map { CGFloat($0) }
    }

    static func storeSectionRatios(_ ratios: [CGFloat], repoPath: String) {
        let raw = ratios.map { Double($0) }
        UserDefaults.standard.set(raw, forKey: ratiosPrefix + token(for: repoPath))
    }

    private static func migrateLegacy(prefix: String, repoPath: String) -> Any? {
        let defaults = UserDefaults.standard
        let legacyKey = prefix + repoPath
        guard let value = defaults.object(forKey: legacyKey) else { return nil }
        defaults.set(value, forKey: prefix + token(for: repoPath))
        defaults.removeObject(forKey: legacyKey)
        return value
    }

    private static func token(for repoPath: String) -> String {
        let digest = SHA256.hash(data: Data(repoPath.utf8))
        return digest.prefix(8).map { String(format: "%02x", $0) }.joined()
    }
}
