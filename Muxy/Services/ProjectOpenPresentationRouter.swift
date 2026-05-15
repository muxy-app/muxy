import Foundation

@MainActor
struct ProjectOpenCustomPickerPresentationAdapter {
    private let presentCustomPicker: () -> Void

    init(notificationCenter: NotificationCenter = .default) {
        presentCustomPicker = {
            notificationCenter.post(name: .openProjectPicker, object: nil)
        }
    }

    init(presentCustomPicker: @escaping () -> Void) {
        self.presentCustomPicker = presentCustomPicker
    }

    func present() {
        presentCustomPicker()
    }
}

@MainActor
struct ProjectOpenFinderPresentationAdapter {
    private let presentFinder: () -> Void

    init(presentFinder: @escaping () -> Void) {
        self.presentFinder = presentFinder
    }

    func present() {
        presentFinder()
    }
}

@MainActor
struct ProjectOpenPresentationRouter {
    let preferences: ProjectPickerPreferences
    let customPicker: ProjectOpenCustomPickerPresentationAdapter
    let finder: ProjectOpenFinderPresentationAdapter

    init(
        preferences: ProjectPickerPreferences = ProjectPickerPreferences(),
        customPicker: ProjectOpenCustomPickerPresentationAdapter = ProjectOpenCustomPickerPresentationAdapter(),
        finder: ProjectOpenFinderPresentationAdapter
    ) {
        self.preferences = preferences
        self.customPicker = customPicker
        self.finder = finder
    }

    func present() {
        switch preferences.mode {
        case .custom:
            customPicker.present()
        case .finder:
            finder.present()
        }
    }
}
