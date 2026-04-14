import Foundation

@MainActor
final class SystemNotificationService {
    static let shared = SystemNotificationService()

    var appState: AppState?

    private init() {}
}
