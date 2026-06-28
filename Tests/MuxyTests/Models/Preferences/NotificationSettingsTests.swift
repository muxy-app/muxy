import Foundation
import Testing

@testable import Muxy

@Suite("NotificationSettings")
struct NotificationSettingsTests {
    @Test("defaults preserve current toast and sound delivery while desktop is off")
    func defaultDeliveryPlan() throws {
        let (defaults, suiteName) = try makeDefaults()
        defer { defaults.removePersistentDomain(forName: suiteName) }

        #expect(NotificationSettings.toastEnabled(defaults: defaults))
        #expect(!NotificationSettings.desktopEnabled(defaults: defaults))
        #expect(NotificationSettings.soundRawValue(defaults: defaults) == NotificationSound.funk.rawValue)
        #expect(NotificationSettings.sound(defaults: defaults) == .funk)
        #expect(NotificationSettings.toastPosition(defaults: defaults) == .topCenter)
        #expect(NotificationSettings.providerEnabled(providerID: "codex", defaults: defaults))

        let plan = NotificationSettings.deliveryPlan(defaults: defaults)
        #expect(plan == NotificationDeliveryPlan(
            showToast: true,
            showDesktop: false,
            soundName: NotificationSound.funk.rawValue
        ))
    }

    @Test("stored delivery preferences override defaults")
    func storedDeliveryPreferences() throws {
        let (defaults, suiteName) = try makeDefaults()
        defer { defaults.removePersistentDomain(forName: suiteName) }
        defaults.set(false, forKey: NotificationSettings.Key.toastEnabled)
        defaults.set(true, forKey: NotificationSettings.Key.desktopEnabled)
        defaults.set(NotificationSound.ping.rawValue, forKey: NotificationSettings.Key.sound)
        defaults.set(ToastPosition.bottomRight.rawValue, forKey: NotificationSettings.Key.toastPosition)
        defaults.set(false, forKey: NotificationSettings.providerEnabledKey(for: "codex"))

        #expect(!NotificationSettings.toastEnabled(defaults: defaults))
        #expect(NotificationSettings.desktopEnabled(defaults: defaults))
        #expect(NotificationSettings.sound(defaults: defaults) == .ping)
        #expect(NotificationSettings.toastPosition(defaults: defaults) == .bottomRight)
        #expect(!NotificationSettings.providerEnabled(providerID: "codex", defaults: defaults))

        let plan = NotificationSettings.deliveryPlan(defaults: defaults)
        #expect(plan == NotificationDeliveryPlan(
            showToast: false,
            showDesktop: true,
            soundName: NotificationSound.ping.rawValue
        ))
    }

    @Test("None sound suppresses sound delivery")
    func noneSoundSuppressesDelivery() throws {
        let (defaults, suiteName) = try makeDefaults()
        defer { defaults.removePersistentDomain(forName: suiteName) }
        defaults.set(NotificationSound.none.rawValue, forKey: NotificationSettings.Key.sound)

        #expect(NotificationSettings.sound(defaults: defaults) == NotificationSound.none)
        #expect(NotificationSettings.deliveryPlan(defaults: defaults).soundName == nil)
    }

    @Test("invalid stored sound raw value is preserved for delivery compatibility")
    func invalidSoundRawValueIsPreserved() throws {
        let (defaults, suiteName) = try makeDefaults()
        defer { defaults.removePersistentDomain(forName: suiteName) }
        defaults.set("Custom", forKey: NotificationSettings.Key.sound)

        #expect(NotificationSettings.sound(defaults: defaults) == nil)
        #expect(NotificationSettings.deliveryPlan(defaults: defaults).soundName == "Custom")
    }

    @Test("invalid toast position falls back to default")
    func invalidToastPositionFallsBack() throws {
        let (defaults, suiteName) = try makeDefaults()
        defer { defaults.removePersistentDomain(forName: suiteName) }
        defaults.set("Middle", forKey: NotificationSettings.Key.toastPosition)

        #expect(NotificationSettings.toastPosition(defaults: defaults) == .topCenter)
    }

    @Test("playableSound returns default sound for missing value")
    func playableSoundDefaultsMissingValue() {
        #expect(NotificationSound.playableSound(for: nil) == .funk)
    }

    @Test("playableSound ignores none")
    func playableSoundIgnoresNone() {
        #expect(NotificationSound.playableSound(for: NotificationSound.none.rawValue) == nil)
    }

    @Test("playableSound ignores unknown values")
    func playableSoundIgnoresUnknownValues() {
        #expect(NotificationSound.playableSound(for: "Custom") == nil)
    }

    @Test("playableSound accepts supported values")
    func playableSoundAcceptsSupportedValues() {
        #expect(NotificationSound.playableSound(for: NotificationSound.ping.rawValue) == .ping)
    }

    private func makeDefaults() throws -> (UserDefaults, String) {
        let suiteName = "NotificationSettingsTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            Issue.record("Unable to create isolated UserDefaults suite")
            throw NotificationSettingsTestError.unavailableDefaults
        }
        defaults.removePersistentDomain(forName: suiteName)
        return (defaults, suiteName)
    }
}

private enum NotificationSettingsTestError: Error {
    case unavailableDefaults
}
