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

    func consume(_ request: SettingsFocusRequest) -> Bool {
        pendingRequests.remove(request) != nil
    }
}

enum SettingsFocusRequest: Hashable {
    case projectPickerDefaultLocation

    var notificationName: Notification.Name {
        switch self {
        case .projectPickerDefaultLocation:
            .focusProjectPickerDefaultLocation
        }
    }
}
