import Foundation

enum TerminalSplitPreferences {
    static let independentTabsKey = "muxy.terminalSplit.independentTabs"
    static let defaultIndependentTabs = false

    static var independentTabs: Bool {
        get {
            let defaults = UserDefaults.standard
            guard defaults.object(forKey: independentTabsKey) != nil else { return defaultIndependentTabs }
            return defaults.bool(forKey: independentTabsKey)
        }
        set { UserDefaults.standard.set(newValue, forKey: independentTabsKey) }
    }
}
