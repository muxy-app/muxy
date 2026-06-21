import Foundation

enum BrowserPreferences {
    static let openLinksInBuiltInBrowserKey = "muxy.browser.openLinksInBuiltIn"
    static let defaultProfileIDKey = "muxy.browser.defaultProfileID"

    static var openLinksInBuiltInBrowser: Bool {
        get { UserDefaults.standard.bool(forKey: openLinksInBuiltInBrowserKey) }
        set { UserDefaults.standard.set(newValue, forKey: openLinksInBuiltInBrowserKey) }
    }

    static var defaultProfileID: UUID {
        get {
            guard let raw = UserDefaults.standard.string(forKey: defaultProfileIDKey),
                  let id = UUID(uuidString: raw)
            else { return BrowserProfile.defaultID }
            return id
        }
        set { UserDefaults.standard.set(newValue.uuidString, forKey: defaultProfileIDKey) }
    }
}
