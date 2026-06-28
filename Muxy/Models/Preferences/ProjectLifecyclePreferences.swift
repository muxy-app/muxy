import Foundation

enum ProjectLifecyclePreferences {
    static let keepOpenWhenNoTabsKey = "muxy.projects.keepOpenWhenNoTabs"

    static var keepOpenWhenNoTabs: Bool {
        get { UserDefaults.standard.bool(forKey: keepOpenWhenNoTabsKey) }
        set { UserDefaults.standard.set(newValue, forKey: keepOpenWhenNoTabsKey) }
    }
}
