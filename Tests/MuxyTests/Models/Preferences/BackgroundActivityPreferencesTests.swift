import Foundation
import Testing

@testable import Muxy

@Suite("Background activity preferences")
struct BackgroundActivityPreferencesTests {
    @Test("defaults to keeping background processes active")
    func defaultsToKeepActive() {
        let defaults = makeDefaults()
        #expect(BackgroundActivityPreferences.keepActive(defaults: defaults))
    }

    @Test("persists changes and only notifies when the effective value changes")
    func persistsAndNotifiesChanges() {
        let defaults = makeDefaults()
        let notificationCenter = NotificationCenter()
        let notifications = NotificationCounter()
        let observer = notificationCenter.addObserver(
            forName: .backgroundActivityKeepActiveDidChange,
            object: defaults,
            queue: nil
        ) { _ in
            notifications.count += 1
        }
        defer { notificationCenter.removeObserver(observer) }

        BackgroundActivityPreferences.setKeepActive(
            false,
            defaults: defaults,
            notificationCenter: notificationCenter
        )
        BackgroundActivityPreferences.setKeepActive(
            false,
            defaults: defaults,
            notificationCenter: notificationCenter
        )
        BackgroundActivityPreferences.setKeepActive(
            true,
            defaults: defaults,
            notificationCenter: notificationCenter
        )

        #expect(BackgroundActivityPreferences.keepActive(defaults: defaults))
        #expect(notifications.count == 2)
    }

    @Test("keep active reports visible regardless of pane or window state")
    func keepActiveAlwaysVisible() {
        #expect(BackgroundActivityPreferences.effectiveVisibility(
            keepActive: true, isPaneVisible: false, isWindowVisible: false
        ))
        #expect(BackgroundActivityPreferences.effectiveVisibility(
            keepActive: true, isPaneVisible: true, isWindowVisible: false
        ))
        #expect(BackgroundActivityPreferences.effectiveVisibility(
            keepActive: true, isPaneVisible: false, isWindowVisible: true
        ))
    }

    @Test("without keep active, visibility follows pane and window state")
    func withoutKeepActiveFollowsState() {
        #expect(BackgroundActivityPreferences.effectiveVisibility(
            keepActive: false, isPaneVisible: true, isWindowVisible: true
        ))
        #expect(!BackgroundActivityPreferences.effectiveVisibility(
            keepActive: false, isPaneVisible: false, isWindowVisible: true
        ))
        #expect(!BackgroundActivityPreferences.effectiveVisibility(
            keepActive: false, isPaneVisible: true, isWindowVisible: false
        ))
        #expect(!BackgroundActivityPreferences.effectiveVisibility(
            keepActive: false, isPaneVisible: false, isWindowVisible: false
        ))
    }

    private func makeDefaults() -> UserDefaults {
        let suiteName = "BackgroundActivityPreferencesTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            fatalError("Unable to create isolated UserDefaults suite")
        }
        defaults.removePersistentDomain(forName: suiteName)
        return defaults
    }
}

private final class NotificationCounter: @unchecked Sendable {
    var count = 0
}
