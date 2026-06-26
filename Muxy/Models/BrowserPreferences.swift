import Foundation

enum BrowserPreferences {
    static let enabledKey = "muxy.browser.enabled"
    static let openLinksInBuiltInBrowserKey = "muxy.browser.openLinksInBuiltIn"
    static let defaultProfileIDKey = "muxy.browser.defaultProfileID"
    static let searchEngineKey = "muxy.browser.searchEngine"
    static let homePageURLKey = "muxy.browser.homePageURL"

    static let defaultSearchEngine = BrowserSearchEngine.google

    static var isEnabled: Bool {
        get {
            guard UserDefaults.standard.object(forKey: enabledKey) != nil else { return true }
            return UserDefaults.standard.bool(forKey: enabledKey)
        }
        set { UserDefaults.standard.set(newValue, forKey: enabledKey) }
    }

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

    static var searchEngine: BrowserSearchEngine {
        get {
            guard let raw = UserDefaults.standard.string(forKey: searchEngineKey),
                  let engine = BrowserSearchEngine(rawValue: raw)
            else { return defaultSearchEngine }
            return engine
        }
        set { UserDefaults.standard.set(newValue.rawValue, forKey: searchEngineKey) }
    }

    static var homePageURLString: String {
        get { UserDefaults.standard.string(forKey: homePageURLKey) ?? BrowserHomePage.blankURLString }
        set { UserDefaults.standard.set(newValue, forKey: homePageURLKey) }
    }

    static var homePageURL: URL? {
        BrowserHomePage.resolvedURL(from: homePageURLString)
    }
}
