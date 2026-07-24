import Foundation
import Testing

@testable import Muxy

@Suite("ProjectPickerTerminalFocusRestoration")
struct ProjectPickerTerminalFocusRestorationTests {
    @Test("successful project open refocuses the terminal after the picker exits")
    func successfulProjectOpenRefocusesTerminalAfterPickerExit() {
        let notificationCenter = NotificationCenter()
        let counter = NotificationCounter(
            notificationCenter: notificationCenter,
            name: .refocusActiveTerminal
        )
        var restoration = ProjectPickerTerminalFocusRestoration()

        restoration.record(.success)
        restoration.overlayExitCompleted(notificationCenter: notificationCenter)

        #expect(counter.count == 1)
        #expect(!restoration.isPending)
    }

    @Test("failed project confirmation does not refocus the terminal")
    func failedProjectConfirmationDoesNotRefocusTerminal() {
        let notificationCenter = NotificationCenter()
        let counter = NotificationCounter(
            notificationCenter: notificationCenter,
            name: .refocusActiveTerminal
        )
        var restoration = ProjectPickerTerminalFocusRestoration()

        restoration.record(.failed)
        restoration.overlayExitCompleted(notificationCenter: notificationCenter)

        #expect(counter.count == 0)
        #expect(!restoration.isPending)
    }

    @Test("focus restoration request is consumed once")
    func focusRestorationRequestIsConsumedOnce() {
        let notificationCenter = NotificationCenter()
        let counter = NotificationCounter(
            notificationCenter: notificationCenter,
            name: .refocusActiveTerminal
        )
        var restoration = ProjectPickerTerminalFocusRestoration()

        restoration.record(.success)
        restoration.overlayExitCompleted(notificationCenter: notificationCenter)
        restoration.overlayExitCompleted(notificationCenter: notificationCenter)

        #expect(counter.count == 1)
    }
}

private final class NotificationCounter: @unchecked Sendable {
    private(set) var count = 0
    private let notificationCenter: NotificationCenter
    private var token: NSObjectProtocol?

    init(notificationCenter: NotificationCenter, name: Notification.Name) {
        self.notificationCenter = notificationCenter
        token = notificationCenter.addObserver(forName: name, object: nil, queue: nil) { [weak self] _ in
            self?.count += 1
        }
    }

    deinit {
        if let token {
            notificationCenter.removeObserver(token)
        }
    }
}
