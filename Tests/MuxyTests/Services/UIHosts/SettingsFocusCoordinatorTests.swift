import Foundation
import Testing

@testable import Muxy

@Suite("SettingsFocusCoordinator")
@MainActor
struct SettingsFocusCoordinatorTests {
    @Test("focus requests are retained, emitted, and consumed once")
    func requestIsOneShot() {
        let notificationCenter = NotificationCenter()
        let coordinator = SettingsFocusCoordinator(notificationCenter: notificationCenter)
        let flag = SettingsFocusNotificationFlag()
        let observer = notificationCenter.addObserver(
            forName: .focusProjectPickerDefaultLocation,
            object: nil,
            queue: nil
        ) { _ in
            flag.didPost = true
        }
        defer { notificationCenter.removeObserver(observer) }

        coordinator.request(.projectPickerDefaultLocation)

        #expect(flag.didPost)
        #expect(coordinator.consume(.projectPickerDefaultLocation))
        #expect(!coordinator.consume(.projectPickerDefaultLocation))
    }

    @Test("notch terminal requests route to shortcut settings")
    func notchTerminalRequestRoutesToShortcutSettings() {
        let notificationCenter = NotificationCenter()
        let coordinator = SettingsFocusCoordinator(notificationCenter: notificationCenter)
        let flag = SettingsFocusNotificationFlag()
        let observer = notificationCenter.addObserver(
            forName: .focusNotchTerminalShortcut,
            object: nil,
            queue: nil
        ) { _ in
            flag.didPost = true
        }
        defer { notificationCenter.removeObserver(observer) }

        coordinator.request(.notchTerminalShortcut)

        #expect(flag.didPost)
        #expect(coordinator.consume(.notchTerminalShortcut))
        #expect(!coordinator.consume(.notchTerminalShortcut))
    }
}

private final class SettingsFocusNotificationFlag: @unchecked Sendable {
    var didPost = false
}
