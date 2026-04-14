import AppKit
import Foundation
import os
import UserNotifications

private let logger = Logger(subsystem: "app.muxy", category: "SystemNotificationService")

@MainActor
final class SystemNotificationService: NSObject {
    static let shared = SystemNotificationService()

    var appState: AppState?

    private var isAvailable = false

    private static let categoryID = "muxy.terminal.notification"

    override private init() {
        super.init()
    }

    func requestPermission() {
        guard Bundle.main.bundleIdentifier != nil else {
            logger.info("Skipping notification setup — no bundle identifier")
            return
        }

        isAvailable = true
        UNUserNotificationCenter.current().delegate = self
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound, .badge]) { granted, error in
            if let error {
                logger.error("Notification permission error: \(error.localizedDescription)")
            }
            logger.info("Notification permission granted: \(granted)")
        }
    }

    func send(_ notification: MuxyNotification) {
        guard isAvailable else {
            logger.debug("System notifications unavailable, skipping OS notification for: \(notification.title)")
            return
        }

        let content = UNMutableNotificationContent()
        content.title = notification.title
        content.body = notification.body
        content.sound = .default
        content.categoryIdentifier = Self.categoryID
        content.userInfo = [
            "notificationID": notification.id.uuidString,
            "projectID": notification.projectID.uuidString,
            "worktreeID": notification.worktreeID.uuidString,
            "areaID": notification.areaID.uuidString,
            "tabID": notification.tabID.uuidString,
            "worktreePath": notification.worktreePath,
        ]

        let request = UNNotificationRequest(
            identifier: notification.id.uuidString,
            content: content,
            trigger: nil
        )

        UNUserNotificationCenter.current().add(request) { error in
            if let error {
                logger.error("Failed to schedule notification: \(error.localizedDescription)")
            }
        }
    }
}

extension SystemNotificationService: @preconcurrency UNUserNotificationCenterDelegate {
    func userNotificationCenter(
        _: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse,
        withCompletionHandler completionHandler: @escaping () -> Void
    ) {
        defer { completionHandler() }

        let userInfo = response.notification.request.content.userInfo
        guard let notificationIDString = userInfo["notificationID"] as? String,
              let notificationID = UUID(uuidString: notificationIDString)
        else { return }

        let store = NotificationStore.shared
        guard let notification = store.notifications.first(where: { $0.id == notificationID }) else { return }

        NSApp.activate()

        guard let appState else { return }
        NotificationNavigator.navigate(to: notification, appState: appState, notificationStore: store)
    }

    func userNotificationCenter(
        _: UNUserNotificationCenter,
        willPresent _: UNNotification,
        withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void
    ) {
        completionHandler([.banner, .sound, .list])
    }
}
