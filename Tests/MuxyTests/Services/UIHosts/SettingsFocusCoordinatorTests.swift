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

    @Test("quick terminal requests route to Quick Terminal settings")
    func quickTerminalRequestRoutesToQuickTerminalSettings() {
        let notificationCenter = NotificationCenter()
        let coordinator = SettingsFocusCoordinator(notificationCenter: notificationCenter)
        let flag = SettingsFocusNotificationFlag()
        let observer = notificationCenter.addObserver(
            forName: .focusQuickTerminalShortcut,
            object: nil,
            queue: nil
        ) { _ in
            flag.didPost = true
        }
        defer { notificationCenter.removeObserver(observer) }

        coordinator.request(.quickTerminalShortcut)

        #expect(flag.didPost)
        #expect(coordinator.consume(.quickTerminalShortcut))
        #expect(!coordinator.consume(.quickTerminalShortcut))
    }

    @Test("opening terminal settings retains focus and emits both notifications")
    func terminalSettingsOpenRoutesToTerminalSettings() {
        let notificationCenter = NotificationCenter()
        let coordinator = SettingsFocusCoordinator(notificationCenter: notificationCenter)
        let flag = SettingsOpenNotificationFlag()
        let focusObserver = notificationCenter.addObserver(
            forName: .focusTerminalSettings,
            object: nil,
            queue: nil
        ) { _ in
            flag.didPostFocus = true
        }
        let openObserver = notificationCenter.addObserver(
            forName: .openSettingsModal,
            object: nil,
            queue: nil
        ) { _ in
            flag.didPostOpen = true
        }
        defer {
            notificationCenter.removeObserver(focusObserver)
            notificationCenter.removeObserver(openObserver)
        }

        coordinator.openSettings(focusedOn: .terminal)

        #expect(flag.didPostFocus)
        #expect(flag.didPostOpen)
        #expect(coordinator.consume(.terminal))
        #expect(!coordinator.consume(.terminal))
    }
}

private final class SettingsFocusNotificationFlag: @unchecked Sendable {
    var didPost = false
}

private final class SettingsOpenNotificationFlag: @unchecked Sendable {
    var didPostFocus = false
    var didPostOpen = false
}
