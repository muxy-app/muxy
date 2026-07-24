import Foundation
import Testing

@testable import Muxy

@Suite("Quick terminal preferences")
struct QuickTerminalPreferencesTests {
    @Test("defaults to enabled")
    func defaultsToEnabled() {
        let defaults = makeDefaults()

        #expect(QuickTerminalPreferences.isEnabled(defaults: defaults))
    }

    @Test("persists changes and only notifies when the effective value changes")
    func persistsAndNotifiesChanges() {
        let defaults = makeDefaults()
        let notificationCenter = NotificationCenter()
        let notifications = QuickTerminalPreferenceNotifications()
        let observer = notificationCenter.addObserver(
            forName: .quickTerminalEnabledDidChange,
            object: defaults,
            queue: nil
        ) { _ in
            notifications.count += 1
        }
        defer { notificationCenter.removeObserver(observer) }

        QuickTerminalPreferences.setEnabled(
            false,
            defaults: defaults,
            notificationCenter: notificationCenter
        )
        QuickTerminalPreferences.setEnabled(
            false,
            defaults: defaults,
            notificationCenter: notificationCenter
        )
        QuickTerminalPreferences.setEnabled(
            true,
            defaults: defaults,
            notificationCenter: notificationCenter
        )

        #expect(QuickTerminalPreferences.isEnabled(defaults: defaults))
        #expect(notifications.count == 2)
    }

    @Test("reset removes the stored value and only notifies when the effective value changes")
    func resetRemovesStoredValueAndNotifiesChanges() {
        let defaults = makeDefaults()
        let notificationCenter = NotificationCenter()
        let notifications = QuickTerminalPreferenceNotifications()
        defaults.set(false, forKey: QuickTerminalPreferences.enabledKey)
        let observer = notificationCenter.addObserver(
            forName: .quickTerminalEnabledDidChange,
            object: defaults,
            queue: nil
        ) { _ in
            notifications.count += 1
        }
        defer { notificationCenter.removeObserver(observer) }

        QuickTerminalPreferences.resetEnabled(
            defaults: defaults,
            notificationCenter: notificationCenter
        )
        QuickTerminalPreferences.resetEnabled(
            defaults: defaults,
            notificationCenter: notificationCenter
        )

        #expect(defaults.object(forKey: QuickTerminalPreferences.enabledKey) == nil)
        #expect(QuickTerminalPreferences.isEnabled(defaults: defaults))
        #expect(notifications.count == 1)
    }

    private func makeDefaults() -> UserDefaults {
        let suiteName = "QuickTerminalPreferencesTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            fatalError("Unable to create isolated UserDefaults suite")
        }
        defaults.removePersistentDomain(forName: suiteName)
        return defaults
    }
}

private final class QuickTerminalPreferenceNotifications: @unchecked Sendable {
    var count = 0
}
