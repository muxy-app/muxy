import Foundation

enum ProjectPickerMode: String, CaseIterable, Identifiable {
    case custom
    case finder

    var id: String { rawValue }

    var label: String {
        switch self {
        case .custom:
            "Custom"
        case .finder:
            "Finder"
        }
    }
}

final class ProjectPickerPreferences {
    static let storageKey = "muxy.projectPicker.mode"
    private let defaults: UserDefaults

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    var mode: ProjectPickerMode {
        get {
            guard let rawValue = defaults.string(forKey: Self.storageKey),
                  let mode = ProjectPickerMode(rawValue: rawValue)
            else { return .custom }
            return mode
        }
        set {
            defaults.set(newValue.rawValue, forKey: Self.storageKey)
        }
    }
}
