import Foundation

enum SidebarPanelPreferences {
    static let rememberPerProjectKey = "muxy.sidebars.rememberPerProject"
    private static let fileTreeStatesKey = "muxy.sidebars.perProject.fileTreeVisible"
    private static let vcsStatesKey = "muxy.sidebars.perProject.vcsVisible"

    static var rememberPerProject: Bool {
        get { UserDefaults.standard.bool(forKey: rememberPerProjectKey) }
        set { UserDefaults.standard.set(newValue, forKey: rememberPerProjectKey) }
    }

    static func fileTreeVisible(for projectID: UUID) -> Bool {
        states(forKey: fileTreeStatesKey)[projectID.uuidString] ?? false
    }

    static func setFileTreeVisible(_ visible: Bool, for projectID: UUID) {
        update(stateKey: fileTreeStatesKey, projectID: projectID, value: visible)
    }

    static func vcsVisible(for projectID: UUID) -> Bool {
        states(forKey: vcsStatesKey)[projectID.uuidString] ?? false
    }

    static func setVCSVisible(_ visible: Bool, for projectID: UUID) {
        update(stateKey: vcsStatesKey, projectID: projectID, value: visible)
    }

    private static func states(forKey key: String) -> [String: Bool] {
        (UserDefaults.standard.dictionary(forKey: key) as? [String: Bool]) ?? [:]
    }

    private static func update(stateKey: String, projectID: UUID, value: Bool) {
        var dict = states(forKey: stateKey)
        dict[projectID.uuidString] = value
        UserDefaults.standard.set(dict, forKey: stateKey)
    }
}
