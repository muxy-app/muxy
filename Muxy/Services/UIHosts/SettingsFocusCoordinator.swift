import Foundation

@MainActor
final class SettingsFocusCoordinator {
    static let shared = SettingsFocusCoordinator()

    private let notificationCenter: NotificationCenter
    private var pendingRequests: Set<SettingsFocusRequest> = []

    init(notificationCenter: NotificationCenter = .default) {
        self.notificationCenter = notificationCenter
    }

    func request(_ request: SettingsFocusRequest) {
        pendingRequests.insert(request)
        notificationCenter.post(name: request.notificationName, object: nil)
    }

    func openSettings(focusedOn request: SettingsFocusRequest) {
        self.request(request)
        notificationCenter.post(name: .openSettingsModal, object: nil)
    }

    func consume(_ request: SettingsFocusRequest) -> Bool {
        pendingRequests.remove(request) != nil
    }
}

enum SettingsFocusRequest: Hashable {
    case projectPickerDefaultLocation
    case remoteDevices
    case browser
    case terminal
    case quickTerminalShortcut

    var notificationName: Notification.Name {
        switch self {
        case .projectPickerDefaultLocation:
            .focusProjectPickerDefaultLocation
        case .remoteDevices:
            .focusRemoteDevicesSettings
        case .browser:
            .focusBrowserSettings
        case .terminal:
            .focusTerminalSettings
        case .quickTerminalShortcut:
            .focusQuickTerminalShortcut
        }
    }
}
