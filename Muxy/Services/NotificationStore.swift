import AppKit
import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "NotificationStore")

@MainActor
@Observable
final class NotificationStore {
    static let shared = NotificationStore()

    var appState: AppState?
    var worktreeStore: WorktreeStore?

    private(set) var notifications: [MuxyNotification] = []

    private static let maxNotifications = 200
    private static let defaults = UserDefaults.standard
    private static let fileURL = MuxyFileStorage.fileURL(filename: "notifications.json")
    private var saveTask: Task<Void, Never>?

    private init() {
        notifications = Self.loadFromDisk()
    }

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
        var changed = false
        for notification in notifications where !notification.isRead && notification.paneID == paneID {
            notification.isRead = true
            changed = true
        }
        if changed { scheduleSave() }
    }

    func markAsRead(areaID: UUID) {
        var changed = false
        for notification in notifications where !notification.isRead && notification.areaID == areaID {
            notification.isRead = true
            changed = true
        }
        if changed { scheduleSave() }
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
        insertIfNotFocused(notification, appState: appState)
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
        insertIfNotFocused(notification, appState: appState)
    }

    private func insertIfNotFocused(_ notification: MuxyNotification, appState: AppState) {
        guard !NSApp.isActive || !NotificationNavigator.isFocused(notification, appState: appState) else {
            return
        }

        notifications.insert(notification, at: 0)
        trimIfNeeded()
        scheduleSave()
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
        scheduleSave()
    }

    func markAllAsRead() {
        var changed = false
        for notification in notifications where !notification.isRead {
            notification.isRead = true
            changed = true
        }
        if changed { scheduleSave() }
    }

    func markAllAsRead(projectID: UUID) {
        var changed = false
        for notification in notifications where !notification.isRead && notification.projectID == projectID {
            notification.isRead = true
            changed = true
        }
        if changed { scheduleSave() }
    }

    func remove(_ id: UUID) {
        notifications.removeAll { $0.id == id }
        scheduleSave()
    }

    func clear() {
        notifications.removeAll()
        scheduleSave()
    }

    private func trimIfNeeded() {
        guard notifications.count > Self.maxNotifications else { return }
        notifications = Array(notifications.prefix(Self.maxNotifications))
    }

    private func scheduleSave() {
        saveTask?.cancel()
        saveTask = Task { [weak self] in
            try? await Task.sleep(for: .seconds(2))
            guard !Task.isCancelled else { return }
            self?.saveToDisk()
        }
    }

    func saveToDisk() {
        do {
            let data = try JSONEncoder().encode(notifications)
            try data.write(to: Self.fileURL, options: .atomic)
        } catch {
            logger.error("Failed to save notifications: \(error.localizedDescription)")
        }
    }

    private static func loadFromDisk() -> [MuxyNotification] {
        guard FileManager.default.fileExists(atPath: fileURL.path) else { return [] }
        do {
            let data = try Data(contentsOf: fileURL)
            let loaded = try JSONDecoder().decode([MuxyNotification].self, from: data)
            return Array(loaded.prefix(maxNotifications))
        } catch {
            logger.error("Failed to load notifications: \(error.localizedDescription)")
            return []
        }
    }
}

extension UserDefaults {
    func bool(forKey key: String, fallback: Bool) -> Bool {
        object(forKey: key) != nil ? bool(forKey: key) : fallback
    }
}
