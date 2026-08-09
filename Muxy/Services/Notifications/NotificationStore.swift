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
    private(set) var readStateVersion: Int = 0

    private static let maxNotifications = 200
    private static let duplicateWindow: TimeInterval = 2
    private static let defaults = UserDefaults.standard
    weak var desktopNotifier: (any DesktopNotificationDelivering)?
    private static let store = CodableFileStore<[MuxyNotification]>(
        fileURL: MuxyFileStorage.fileURL(filename: "notifications.json")
    )
    private var saveTask: Task<Void, Never>?
    private var pendingDesktopDeliveries: [DesktopDelivery] = []

    enum DesktopDeliveryIngress: Equatable {
        case aiHook(providerID: String)
        case terminalOSC
    }

    private struct DesktopDelivery {
        let ingress: DesktopDeliveryIngress
        let projectID: UUID
        let worktreeID: UUID
        let areaID: UUID
        let tabID: UUID
        let title: String
        let body: String
        let timestamp: Date

        init(notification: MuxyNotification, ingress: DesktopDeliveryIngress) {
            self.ingress = ingress
            projectID = notification.projectID
            worktreeID = notification.worktreeID
            areaID = notification.areaID
            tabID = notification.tabID
            title = Self.normalizedTitle(notification.title)
            body = notification.body
            timestamp = notification.timestamp
        }

        private static func normalizedTitle(_ title: String) -> String {
            switch title {
            case "Task completed!",
                 "Command executed!": "completion"
            default: title
            }
        }
    }

    private init() {
        notifications = Self.loadFromDisk()
        NotificationCenter.default.addObserver(
            forName: NSApplication.didBecomeActiveNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor in
                self?.markActiveTabAsRead()
            }
        }
    }

    private func markActiveTabAsRead() {
        guard let appState, let tabID = NotificationNavigator.activeTabID(appState: appState) else { return }
        markAsRead(tabID: tabID)
    }

    var unreadCount: Int {
        _ = readStateVersion
        return notifications.count { !$0.isRead }
    }

    func unreadCount(for projectID: UUID) -> Int {
        _ = readStateVersion
        return notifications.count { !$0.isRead && $0.projectID == projectID }
    }

    func unreadCount(for projectID: UUID, worktreeID: UUID) -> Int {
        _ = readStateVersion
        return notifications.count { !$0.isRead && $0.projectID == projectID && $0.worktreeID == worktreeID }
    }

    func hasUnread(tabID: UUID) -> Bool {
        _ = readStateVersion
        return notifications.contains { !$0.isRead && $0.tabID == tabID }
    }

    func markAsRead(tabID: UUID) {
        var changed = false
        for notification in notifications where !notification.isRead && notification.tabID == tabID {
            notification.isRead = true
            changed = true
        }
        if changed {
            readStateVersion += 1
            scheduleSave()
        }
    }

    func add(
        paneID: UUID,
        source: MuxyNotification.Source,
        title: String,
        body: String,
        appState: AppState,
        desktopDeliveryIngress: DesktopDeliveryIngress? = nil
    ) {
        guard let worktreeStore else {
            logger.debug("Notification dropped: worktreeStore not set")
            return
        }
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
        insertIfNotFocused(
            notification,
            appState: appState,
            desktopDeliveryIngress: desktopDeliveryIngress
        )
    }

    func addWithContext(
        context: NavigationContext,
        source: MuxyNotification.Source,
        title: String,
        body: String,
        appState: AppState,
        timestamp: Date = Date(),
        desktopDeliveryIngress: DesktopDeliveryIngress? = nil
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
            body: body,
            timestamp: timestamp
        )
        insertIfNotFocused(
            notification,
            appState: appState,
            desktopDeliveryIngress: desktopDeliveryIngress
        )
    }

    private func insertIfNotFocused(
        _ notification: MuxyNotification,
        appState: AppState,
        desktopDeliveryIngress: DesktopDeliveryIngress?
    ) {
        if notification.source == .osc,
           NSApplication.shared.isActive,
           NotificationNavigator.isActiveTab(notification.tabID, appState: appState)
        {
            playSound()
            return
        }

        notifications.insert(notification, at: 0)
        trimIfNeeded()
        scheduleSave()
        deliverNotification(notification, desktopDeliveryIngress: desktopDeliveryIngress)
        broadcastExtensionEvent(notification)
    }

    private func broadcastExtensionEvent(_ notification: MuxyNotification) {
        NotificationSocketServer.shared.broadcast(event: ExtensionEvent(
            name: ExtensionEventName.notificationPosted,
            payload: [
                "paneID": notification.paneID.uuidString,
                "projectID": notification.projectID.uuidString,
                "worktreeID": notification.worktreeID.uuidString,
                "worktreePath": notification.worktreePath,
                "tabID": notification.tabID.uuidString,
                "source": notification.source.key,
                "title": notification.title,
                "body": notification.body,
            ]
        ))
    }

    private func deliverNotification(
        _ notification: MuxyNotification,
        desktopDeliveryIngress: DesktopDeliveryIngress?
    ) {
        let plan = NotificationSettings.deliveryPlan(defaults: Self.defaults)
        if plan.showToast {
            ToastState.shared.show(title: notification.title, body: notification.body) { [weak self] in
                self?.activate(notificationID: notification.id)
            }
        }
        if plan.showDesktop, shouldDeliverDesktopNotification(notification, ingress: desktopDeliveryIngress) {
            desktopNotifier?.deliver(notification)
        }
        playSound()
    }

    private func shouldDeliverDesktopNotification(
        _ notification: MuxyNotification,
        ingress: DesktopDeliveryIngress?
    ) -> Bool {
        guard let ingress else { return true }
        let delivery = DesktopDelivery(notification: notification, ingress: ingress)
        pendingDesktopDeliveries.removeAll {
            abs($0.timestamp.timeIntervalSince(delivery.timestamp)) > Self.duplicateWindow
        }
        let matches = pendingDesktopDeliveries.indices.filter {
            Self.isComplementaryDesktopDeliveryPair(pendingDesktopDeliveries[$0], delivery)
        }
        guard matches.count == 1, let match = matches.first else {
            guard matches.isEmpty else { return true }
            pendingDesktopDeliveries.append(delivery)
            return true
        }
        pendingDesktopDeliveries.remove(at: match)
        return false
    }

    private static func isComplementaryDesktopDeliveryPair(
        _ lhs: DesktopDelivery,
        _ rhs: DesktopDelivery
    ) -> Bool {
        lhs.projectID == rhs.projectID &&
            lhs.worktreeID == rhs.worktreeID &&
            lhs.areaID == rhs.areaID &&
            lhs.tabID == rhs.tabID &&
            lhs.title == rhs.title &&
            lhs.body == rhs.body &&
            abs(lhs.timestamp.timeIntervalSince(rhs.timestamp)) <= duplicateWindow &&
            ((lhs.ingress == .terminalOSC && rhs.ingress.isAIHook) ||
                (lhs.ingress.isAIHook && rhs.ingress == .terminalOSC))
    }

    private func activate(notificationID: UUID) {
        guard let appState else { return }
        NSApp.activate()
        guard NotificationNavigator.navigate(
            notificationID: notificationID,
            appState: appState,
            notificationStore: self
        )
        else {
            logger.debug("Toast notification response ignored: notification not found")
            return
        }
    }

    private func playSound() {
        guard let soundName = NotificationSettings.deliveryPlan(defaults: Self.defaults).soundName else { return }
        guard let sound = NotificationSound.playableSound(for: soundName) else { return }
        NotificationSoundPlayer.shared.play(sound)
    }

    func markAsRead(_ id: UUID) {
        guard let index = notifications.firstIndex(where: { $0.id == id }) else { return }
        notifications[index].isRead = true
        readStateVersion += 1
        scheduleSave()
    }

    func markAllAsRead() {
        var changed = false
        for notification in notifications where !notification.isRead {
            notification.isRead = true
            changed = true
        }
        if changed {
            readStateVersion += 1
            scheduleSave()
        }
    }

    func markAllAsRead(projectID: UUID) {
        var changed = false
        for notification in notifications where !notification.isRead && notification.projectID == projectID {
            notification.isRead = true
            changed = true
        }
        if changed {
            readStateVersion += 1
            scheduleSave()
        }
    }

    func remove(_ id: UUID) {
        notifications.removeAll { $0.id == id }
        scheduleSave()
    }

    func clear() {
        notifications.removeAll()
        pendingDesktopDeliveries.removeAll()
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
            try Self.store.save(notifications)
        } catch {
            logger.error("Failed to save notifications: \(error.localizedDescription)")
        }
    }

    private static func loadFromDisk() -> [MuxyNotification] {
        do {
            let loaded = try store.load() ?? []
            return Array(loaded.prefix(maxNotifications))
        } catch {
            logger.error("Failed to load notifications: \(error.localizedDescription)")
            return []
        }
    }
}

private extension NotificationStore.DesktopDeliveryIngress {
    var isAIHook: Bool {
        if case .aiHook = self {
            return true
        }
        return false
    }
}
