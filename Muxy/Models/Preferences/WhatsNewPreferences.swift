import Foundation

enum WhatsNewPreferences {
    static let viewedVersionKey = "muxy.whatsNew.viewedVersion"

    static var currentVersion: String? {
        #if DEBUG
        if let override = ProcessInfo.processInfo.environment["MUXY_WHATS_NEW_VERSION"],
           !override.isEmpty
        {
            return override
        }
        #endif
        return Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String
    }

    static var viewedVersion: String? {
        get { UserDefaults.standard.string(forKey: viewedVersionKey) }
        set { UserDefaults.standard.set(newValue, forKey: viewedVersionKey) }
    }

    static var shouldAutoShow: Bool {
        guard let currentVersion else { return false }
        return currentVersion != viewedVersion
    }

    static func markCurrentVersionViewed() {
        guard let currentVersion else { return }
        viewedVersion = currentVersion
    }
}
