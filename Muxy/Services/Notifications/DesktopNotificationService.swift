import AppKit
import Foundation
import os
@preconcurrency import UserNotifications

private let desktopNotificationLogger = Logger(subsystem: "app.muxy", category: "DesktopNotificationService")

struct DesktopNotificationPayload: Equatable {
    static let notificationIDUserInfoKey = "notificationID"

    let notificationID: UUID
    let title: String
    let body: String

    init(notification: MuxyNotification) {
        notificationID = notification.id
        title = notification.title
        body = notification.body
    }

    var requestIdentifier: String {
        notificationID.uuidString
    }

    func makeRequest() -> UNNotificationRequest {
        let content = UNMutableNotificationContent()
        content.title = title
        content.body = body
        content.userInfo = [Self.notificationIDUserInfoKey: notificationID.uuidString]
        return UNNotificationRequest(identifier: requestIdentifier, content: content, trigger: nil)
    }

    static func notificationID(from userInfo: [AnyHashable: Any]) -> UUID? {
        guard let rawValue = userInfo[notificationIDUserInfoKey] as? String else { return nil }
        return UUID(uuidString: rawValue)
    }
}

@MainActor
protocol DesktopNotificationDelivering: AnyObject {
    func deliver(_ notification: MuxyNotification)
}

protocol UserNotificationScheduling: AnyObject {
    var delegate: (any UNUserNotificationCenterDelegate)? { get set }

    func authorizationStatus(completionHandler: @escaping @Sendable (UNAuthorizationStatus) -> Void)
    func requestAuthorization(
        options: UNAuthorizationOptions,
        completionHandler: @escaping @Sendable (Bool, (any Error)?) -> Void
    )
    func add(_ request: UNNotificationRequest, withCompletionHandler completionHandler: (@Sendable ((any Error)?) -> Void)?)
}

final class SystemUserNotificationScheduler: UserNotificationScheduling {
    private let centerProvider: () -> UNUserNotificationCenter?
    private lazy var center: UNUserNotificationCenter? = centerProvider()

    init(centerProvider: @escaping () -> UNUserNotificationCenter? = SystemUserNotificationScheduler.defaultCenter) {
        self.centerProvider = centerProvider
    }

    var delegate: (any UNUserNotificationCenterDelegate)? {
        get { center?.delegate }
        set { center?.delegate = newValue }
    }

    func authorizationStatus(completionHandler: @escaping @Sendable (UNAuthorizationStatus) -> Void) {
        guard let center else {
            completionHandler(.denied)
            return
        }
        center.getNotificationSettings { settings in
            completionHandler(settings.authorizationStatus)
        }
    }

    func requestAuthorization(
        options: UNAuthorizationOptions,
        completionHandler: @escaping @Sendable (Bool, (any Error)?) -> Void
    ) {
        guard let center else {
            completionHandler(false, nil)
            return
        }
        center.requestAuthorization(options: options, completionHandler: completionHandler)
    }

    func add(_ request: UNNotificationRequest, withCompletionHandler completionHandler: (@Sendable ((any Error)?) -> Void)?) {
        guard let center else {
            completionHandler?(nil)
            return
        }
        center.add(request, withCompletionHandler: completionHandler)
    }

    private static func defaultCenter() -> UNUserNotificationCenter? {
        guard Bundle.main.bundleIdentifier != nil else { return nil }
        return .current()
    }
}

@MainActor
final class DesktopNotificationService: NSObject, DesktopNotificationDelivering {
    static let shared = DesktopNotificationService()

    private let scheduler: any UserNotificationScheduling
    private var appState: AppState?
    private var notificationStore: NotificationStore?
    private var pendingNavigationIDs: [UUID] = []
    private var isStarted = false

    init(scheduler: any UserNotificationScheduling = SystemUserNotificationScheduler()) {
        self.scheduler = scheduler
        super.init()
    }

    func prepare() {
        guard !isStarted else { return }
        scheduler.delegate = self
        isStarted = true
    }

    func start(appState: AppState, notificationStore: NotificationStore = .shared) {
        prepare()
        notificationStore.desktopNotifier = self
        self.appState = appState
        self.notificationStore = notificationStore
        flushPendingNavigation()
    }

    func deliver(_ notification: MuxyNotification) {
        schedule(DesktopNotificationPayload(notification: notification))
    }

    func requestAuthorizationIfNeeded(completion: (@MainActor (Bool) -> Void)? = nil) {
        scheduler.authorizationStatus { [weak self] status in
            Task { @MainActor in
                self?.requestAuthorizationIfNeeded(status: status, completion: completion)
            }
        }
    }

    nonisolated func foregroundPresentationOptions() -> UNNotificationPresentationOptions {
        []
    }

    nonisolated func notificationIDForResponse(actionIdentifier: String, from userInfo: [AnyHashable: Any]) -> UUID? {
        guard actionIdentifier == UNNotificationDefaultActionIdentifier else { return nil }
        return DesktopNotificationPayload.notificationID(from: userInfo)
    }

    nonisolated func handleNotificationResponse(
        actionIdentifier: String,
        userInfo: [AnyHashable: Any],
        completionHandler: @escaping @Sendable () -> Void
    ) {
        guard let notificationID = notificationIDForResponse(actionIdentifier: actionIdentifier, from: userInfo) else {
            completionHandler()
            return
        }
        Task { @MainActor [weak self] in
            self?.handleResponse(notificationID: notificationID)
            completionHandler()
        }
    }

    private func requestAuthorizationIfNeeded(
        status: UNAuthorizationStatus,
        completion: (@MainActor (Bool) -> Void)?
    ) {
        switch status {
        case .authorized,
             .provisional,
             .ephemeral:
            completion?(true)
        case .notDetermined:
            requestAuthorization(completion: completion)
        case .denied:
            completion?(false)
        @unknown default:
            completion?(false)
        }
    }

    private func schedule(_ payload: DesktopNotificationPayload) {
        scheduler.authorizationStatus { [weak self] status in
            Task { @MainActor in
                self?.schedule(payload, authorizationStatus: status)
            }
        }
    }

    private func schedule(_ payload: DesktopNotificationPayload, authorizationStatus: UNAuthorizationStatus) {
        switch authorizationStatus {
        case .authorized,
             .provisional,
             .ephemeral:
            add(payload)
        case .notDetermined:
            desktopNotificationLogger.debug("Desktop notification skipped: authorization not determined")
        case .denied:
            desktopNotificationLogger.debug("Desktop notification skipped: authorization denied")
        @unknown default:
            desktopNotificationLogger.debug("Desktop notification skipped: unknown authorization status")
        }
    }

    private func requestAuthorization(completion: (@MainActor (Bool) -> Void)? = nil) {
        scheduler.requestAuthorization(options: [.alert]) { granted, error in
            if let error {
                desktopNotificationLogger.error("Desktop notification authorization failed: \(error.localizedDescription)")
            }
            Task { @MainActor in
                completion?(granted)
            }
        }
    }

    private func add(_ payload: DesktopNotificationPayload) {
        scheduler.add(payload.makeRequest()) { error in
            if let error {
                desktopNotificationLogger.error("Desktop notification scheduling failed: \(error.localizedDescription)")
            }
        }
    }

    private func handleResponse(notificationID: UUID) {
        guard let appState, let notificationStore else {
            pendingNavigationIDs.append(notificationID)
            return
        }
        NSApp.activate()
        guard NotificationNavigator.navigate(
            notificationID: notificationID,
            appState: appState,
            notificationStore: notificationStore
        )
        else {
            desktopNotificationLogger.debug("Desktop notification response ignored: notification not found")
            return
        }
    }

    private func flushPendingNavigation() {
        guard !pendingNavigationIDs.isEmpty else { return }
        let ids = pendingNavigationIDs
        pendingNavigationIDs.removeAll()
        for id in ids {
            handleResponse(notificationID: id)
        }
    }
}

extension DesktopNotificationService: UNUserNotificationCenterDelegate {
    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification,
        withCompletionHandler completionHandler: @escaping @Sendable (UNNotificationPresentationOptions) -> Void
    ) {
        completionHandler(foregroundPresentationOptions())
    }

    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse,
        withCompletionHandler completionHandler: @escaping @Sendable () -> Void
    ) {
        handleNotificationResponse(
            actionIdentifier: response.actionIdentifier,
            userInfo: response.notification.request.content.userInfo,
            completionHandler: completionHandler
        )
    }
}
