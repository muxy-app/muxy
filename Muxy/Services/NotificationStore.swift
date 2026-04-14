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
    private static let defaults = UserDefaults.standard

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
        guard let worktreeStore else { return }
        guard let context = NotificationNavigator.resolveContext(
            for: paneID,
            appState: appState,
            worktreeStore: worktreeStore
        )
        else { return }

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

        if NSApp.isActive, NotificationNavigator.isFocused(notification, appState: appState) {
            return
        }

        notifications.insert(notification, at: 0)
        trimIfNeeded()
        deliverNotification(notification)
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

        if NSApp.isActive, NotificationNavigator.isFocused(notification, appState: appState) {
            return
        }

        notifications.insert(notification, at: 0)
        trimIfNeeded()
        deliverNotification(notification)
    }

    private func deliverNotification(_ notification: MuxyNotification) {
        if Self.defaults.bool(forKey: "muxy.notifications.toastEnabled", fallback: true) {
            ToastState.shared.show(notification.title)
        }
        playSound()
    }

    private func playSound() {
        let soundName = Self.defaults.string(forKey: "muxy.notifications.sound") ?? NotificationSound.funk.rawValue
        guard soundName != NotificationSound.none.rawValue else { return }
        NSSound(named: .init(soundName))?.play()
    }

    var autoClearDuration: Double? {
        let raw = Self.defaults.string(forKey: "muxy.notifications.autoClear") ?? AutoClearDuration.off.rawValue
        return AutoClearDuration(rawValue: raw)?.seconds
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

extension UserDefaults {
    func bool(forKey key: String, fallback: Bool) -> Bool {
        object(forKey: key) != nil ? bool(forKey: key) : fallback
    }
}
