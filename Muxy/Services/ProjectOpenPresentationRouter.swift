import Foundation

enum ProjectOpenPresentationRoute: Equatable {
    case customPicker
    case finder
}

struct ProjectOpenPresentationRouter {
    let preferences: ProjectPickerPreferences

    init(preferences: ProjectPickerPreferences = ProjectPickerPreferences()) {
        self.preferences = preferences
    }

    func route() -> ProjectOpenPresentationRoute {
        switch preferences.mode {
        case .custom:
            .customPicker
        case .finder:
            .finder
        }
    }
}
