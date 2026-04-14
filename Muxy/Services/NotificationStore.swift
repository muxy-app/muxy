import AppKit
import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "NotificationStore")

@MainActor
@Observable
final class NotificationStore {
    static let shared = NotificationStore()

    var worktreeStore: WorktreeStore?

    private(set) var notifications: [MuxyNotification] = []

    private static let maxNotifications = 200

    private init() {}

    var unreadCount: Int {
        notifications.count { !$0.isRead }
    }

    func unreadCount(for projectID: UUID) -> Int {
        notifications.count { !$0.isRead && $0.projectID == projectID }
    }

    func unreadCount(for projectID: UUID, worktreeID: UUID) -> Int {
        notifications.count { !$0.isRead && $0.projectID == projectID && $0.worktreeID == worktreeID }
    }

    func markAsRead(paneID: UUID) {
        for notification in notifications where !notification.isRead && notification.paneID == paneID {
            notification.isRead = true
        }
    }

    func markAsRead(areaID: UUID) {
        for notification in notifications where !notification.isRead && notification.areaID == areaID {
            notification.isRead = true
        }
    }

    func add(
        paneID: UUID,
        source: MuxyNotification.Source,
        title: String,
        body: String,
        appState: AppState
    ) {
        guard let worktreeStore else {
            print("[Muxy] Dropped notification — worktreeStore not set: \(title)")
            return
        }
        guard let context = NotificationNavigator.resolveContext(
            for: paneID,
            appState: appState,
            worktreeStore: worktreeStore
        )
        else {
            print("[Muxy] Dropped notification — could not resolve context for pane \(paneID): \(title)")
            return
        }
        print("[Muxy] Notification added: \(title) for project=\(context.projectID)")

        let notification = MuxyNotification(
            paneID: paneID,
            projectID: context.projectID,
            worktreeID: context.worktreeID,
            areaID: context.areaID,
            tabID: context.tabID,
            worktreePath: context.worktreePath,
            source: source,
            title: title,
            body: body
        )

        notifications.insert(notification, at: 0)
        trimIfNeeded()

        deliverNotification(notification, appState: appState)
    }

    func addWithContext(
        context: NavigationContext,
        source: MuxyNotification.Source,
        title: String,
        body: String,
        appState: AppState
    ) {
        let notification = MuxyNotification(
            paneID: UUID(),
            projectID: context.projectID,
            worktreeID: context.worktreeID,
            areaID: context.areaID,
            tabID: context.tabID,
            worktreePath: context.worktreePath,
            source: source,
            title: title,
            body: body
        )

        notifications.insert(notification, at: 0)
        trimIfNeeded()

        deliverNotification(notification, appState: appState)
    }

    func addForProject(
        projectPath: String,
        title: String,
        body: String
    ) {
        guard let appState = SystemNotificationService.shared.appState else { return }
        guard let context = NotificationNavigator.resolveContext(
            for: projectPath,
            appState: appState
        )
        else { return }
        addWithContext(
            context: context,
            source: .vcs,
            title: title,
            body: body,
            appState: appState
        )
    }

    private func deliverNotification(_ notification: MuxyNotification, appState: AppState) {
        let suppressBanner = NSApp.isActive && NotificationNavigator.isFocused(notification, appState: appState)
        ToastState.shared.show(notification.title)
        playSound()
        if !suppressBanner {
            SystemNotificationService.shared.send(notification)
        }
    }

    private func playSound() {
        NSSound(named: .init("Funk"))?.play()
    }

    func markAsRead(_ id: UUID) {
        guard let index = notifications.firstIndex(where: { $0.id == id }) else { return }
        notifications[index].isRead = true
    }

    func markAllAsRead() {
        for notification in notifications where !notification.isRead {
            notification.isRead = true
        }
    }

    func markAllAsRead(projectID: UUID) {
        for notification in notifications where !notification.isRead && notification.projectID == projectID {
            notification.isRead = true
        }
    }

    func remove(_ id: UUID) {
        notifications.removeAll { $0.id == id }
    }

    func clear() {
        notifications.removeAll()
    }

    private func trimIfNeeded() {
        guard notifications.count > Self.maxNotifications else { return }
        notifications = Array(notifications.prefix(Self.maxNotifications))
    }
}
