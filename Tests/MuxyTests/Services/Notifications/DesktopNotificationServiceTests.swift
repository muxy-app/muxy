import Foundation
import Testing
import UserNotifications

@testable import Muxy

@Suite("DesktopNotificationService")
@MainActor
struct DesktopNotificationServiceTests {
    @Test("payload builds an immediate request with visible content and minimal userInfo")
    func payloadBuildsRequest() {
        let notification = makeNotification(title: "Build finished", body: "All tests passed")
        let payload = DesktopNotificationPayload(notification: notification)
        let request = payload.makeRequest()

        #expect(request.identifier == notification.id.uuidString)
        #expect(request.trigger == nil)
        #expect(request.content.title == "Build finished")
        #expect(request.content.body == "All tests passed")
        #expect(request.content.userInfo.count == 1)
        #expect(DesktopNotificationPayload.notificationID(from: request.content.userInfo) == notification.id)
    }

    @Test("invalid payload userInfo does not resolve a notification ID")
    func invalidPayloadUserInfo() {
        #expect(DesktopNotificationPayload.notificationID(from: [:]) == nil)
        #expect(DesktopNotificationPayload.notificationID(from: [
            DesktopNotificationPayload.notificationIDUserInfoKey: "not-a-uuid",
        ]) == nil)
    }

    @Test("foreground presentation is suppressed while the app owns in-app notification UI")
    func foregroundPresentationIsSuppressed() {
        let service = DesktopNotificationService(scheduler: UserNotificationSchedulerSpy())

        #expect(service.foregroundPresentationOptions().isEmpty)
    }

    @Test("response handling only accepts default actions with a valid notification ID")
    func responseHandlingFiltersActionsAndPayloads() {
        let service = DesktopNotificationService(scheduler: UserNotificationSchedulerSpy())
        let notification = makeNotification()

        #expect(service.notificationIDForResponse(
            actionIdentifier: UNNotificationDefaultActionIdentifier,
            from: [
                DesktopNotificationPayload.notificationIDUserInfoKey: notification.id.uuidString,
            ]
        ) == notification.id)
        #expect(service.notificationIDForResponse(
            actionIdentifier: UNNotificationDismissActionIdentifier,
            from: [
                DesktopNotificationPayload.notificationIDUserInfoKey: notification.id.uuidString,
            ]
        ) == nil)
        #expect(service.notificationIDForResponse(
            actionIdentifier: UNNotificationDefaultActionIdentifier,
            from: [
                DesktopNotificationPayload.notificationIDUserInfoKey: "not-a-uuid",
            ]
        ) == nil)
    }

    @Test("notification responses always complete exactly once")
    func notificationResponsesCompleteExactlyOnce() async {
        let service = DesktopNotificationService(scheduler: UserNotificationSchedulerSpy())
        let notification = makeNotification()
        let validCompletion = CompletionRecorder()
        let dismissedCompletion = CompletionRecorder()
        let invalidCompletion = CompletionRecorder()

        service.handleNotificationResponse(
            actionIdentifier: UNNotificationDefaultActionIdentifier,
            userInfo: [
                DesktopNotificationPayload.notificationIDUserInfoKey: notification.id.uuidString,
            ]
        ) {
            validCompletion.record()
        }
        service.handleNotificationResponse(
            actionIdentifier: UNNotificationDismissActionIdentifier,
            userInfo: [
                DesktopNotificationPayload.notificationIDUserInfoKey: notification.id.uuidString,
            ]
        ) {
            dismissedCompletion.record()
        }
        service.handleNotificationResponse(
            actionIdentifier: UNNotificationDefaultActionIdentifier,
            userInfo: [
                DesktopNotificationPayload.notificationIDUserInfoKey: "not-a-uuid",
            ]
        ) {
            invalidCompletion.record()
        }
        await waitUntil {
            validCompletion.count == 1 &&
                dismissedCompletion.count == 1 &&
                invalidCompletion.count == 1
        }

        #expect(validCompletion.count == 1)
        #expect(dismissedCompletion.count == 1)
        #expect(invalidCompletion.count == 1)
    }

    @Test("prepare installs the notification center delegate once")
    func prepareInstallsDelegate() {
        let scheduler = UserNotificationSchedulerSpy()
        let service = DesktopNotificationService(scheduler: scheduler)

        service.prepare()
        let firstDelegate = scheduler.delegate
        service.prepare()

        #expect(firstDelegate != nil)
        #expect(scheduler.delegate === firstDelegate)
    }

    @Test("authorized delivery schedules one local notification")
    func authorizedDeliverySchedulesRequest() async {
        let scheduler = UserNotificationSchedulerSpy(authorizationStatus: .authorized)
        let service = DesktopNotificationService(scheduler: scheduler)
        let notification = makeNotification()

        service.deliver(notification)
        await Task.yield()

        #expect(scheduler.requests.count == 1)
        #expect(scheduler.requests.first?.identifier == notification.id.uuidString)
        #expect(scheduler.authorizationRequests.isEmpty)
    }

    @Test("undetermined delivery waits for settings to request permission")
    func undeterminedDeliveryWaitsForSettingsAuthorization() async {
        let scheduler = UserNotificationSchedulerSpy(authorizationStatus: .notDetermined, grantsAuthorization: true)
        let service = DesktopNotificationService(scheduler: scheduler)

        service.deliver(makeNotification())
        await Task.yield()

        #expect(scheduler.authorizationRequests.isEmpty)
        #expect(scheduler.requests.isEmpty)
    }

    @Test("denied delivery does not schedule")
    func deniedDeliveryDoesNotSchedule() async {
        let scheduler = UserNotificationSchedulerSpy(authorizationStatus: .denied)
        let service = DesktopNotificationService(scheduler: scheduler)

        service.deliver(makeNotification())
        await Task.yield()

        #expect(scheduler.requests.isEmpty)
        #expect(scheduler.authorizationRequests.isEmpty)
    }

    @Test("settings authorization callback reports denied permission")
    func settingsAuthorizationReportsDeniedPermission() async throws {
        let scheduler = UserNotificationSchedulerSpy(authorizationStatus: .notDetermined, grantsAuthorization: false)
        let service = DesktopNotificationService(scheduler: scheduler)
        var result: Bool?

        service.requestAuthorizationIfNeeded { authorized in
            result = authorized
        }
        await waitUntil { result != nil }

        #expect(scheduler.authorizationRequests == [.alert])
        #expect(result == false)
    }

    private func waitUntil(_ predicate: () -> Bool) async {
        for _ in 0 ..< 20 {
            if predicate() { return }
            await Task.yield()
        }
    }

    private func makeNotification(title: String = "Task completed", body: String = "Done") -> MuxyNotification {
        MuxyNotification(
            paneID: UUID(),
            projectID: UUID(),
            worktreeID: UUID(),
            areaID: UUID(),
            tabID: UUID(),
            worktreePath: "/tmp/muxy",
            source: .socket,
            title: title,
            body: body
        )
    }
}

private final class CompletionRecorder: @unchecked Sendable {
    private let lock = NSLock()
    private var storedCount = 0

    var count: Int {
        lock.lock()
        defer { lock.unlock() }
        return storedCount
    }

    func record() {
        lock.lock()
        storedCount += 1
        lock.unlock()
    }
}

private final class UserNotificationSchedulerSpy: UserNotificationScheduling {
    var delegate: (any UNUserNotificationCenterDelegate)?
    var authorizationStatus: UNAuthorizationStatus
    var grantsAuthorization: Bool
    var authorizationRequests: [UNAuthorizationOptions] = []
    var requests: [UNNotificationRequest] = []

    init(authorizationStatus: UNAuthorizationStatus = .authorized, grantsAuthorization: Bool = true) {
        self.authorizationStatus = authorizationStatus
        self.grantsAuthorization = grantsAuthorization
    }

    func authorizationStatus(completionHandler: @escaping @Sendable (UNAuthorizationStatus) -> Void) {
        completionHandler(authorizationStatus)
    }

    func requestAuthorization(
        options: UNAuthorizationOptions,
        completionHandler: @escaping @Sendable (Bool, (any Error)?) -> Void
    ) {
        authorizationRequests.append(options)
        if grantsAuthorization {
            authorizationStatus = .authorized
        }
        completionHandler(grantsAuthorization, nil)
    }

    func add(_ request: UNNotificationRequest, withCompletionHandler completionHandler: (@Sendable ((any Error)?) -> Void)?) {
        requests.append(request)
        completionHandler?(nil)
    }
}
