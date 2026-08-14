import Foundation

struct PanelModePreferences {
    static let storageKeyPrefix = "muxy.panel.mode."

    private let defaults: UserDefaults

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    func mode(for panelID: String, default defaultMode: PanelMode) -> PanelMode {
        let key = storageKey(for: panelID)
        if let rawValue = defaults.string(forKey: key),
           let mode = PanelMode(rawValue: rawValue)
        {
            return mode
        }
        guard panelID == BuiltinPanel.richInput,
              defaults.object(forKey: RichInputPreferences.panelFloatingKey) != nil
        else { return defaultMode }
        let mode: PanelMode = defaults.bool(forKey: RichInputPreferences.panelFloatingKey) ? .floating : .pinned
        defaults.set(mode.rawValue, forKey: key)
        return mode
    }

    func setMode(_ mode: PanelMode, for panelID: String) {
        defaults.set(mode.rawValue, forKey: storageKey(for: panelID))
    }

    func storageKey(for panelID: String) -> String {
        "\(Self.storageKeyPrefix)\(panelID)"
    }
}
