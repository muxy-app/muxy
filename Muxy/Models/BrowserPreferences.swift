import Foundation

enum BrowserPreferences {
    static let openLinksInBuiltInBrowserKey = "muxy.browser.openLinksInBuiltIn"

    static var openLinksInBuiltInBrowser: Bool {
        get { UserDefaults.standard.bool(forKey: openLinksInBuiltInBrowserKey) }
        set { UserDefaults.standard.set(newValue, forKey: openLinksInBuiltInBrowserKey) }
    }
}
