import Foundation

enum NotificationSound: String, CaseIterable, Identifiable {
    case none = "None"
    case basso = "Basso"
    case blow = "Blow"
    case bottle = "Bottle"
    case frog = "Frog"
    case funk = "Funk"
    case glass = "Glass"
    case hero = "Hero"
    case morse = "Morse"
    case ping = "Ping"
    case pop = "Pop"
    case purr = "Purr"
    case sosumi = "Sosumi"
    case submarine = "Submarine"
    case tink = "Tink"

    var id: String { rawValue }

    static func playableSound(for value: String?) -> NotificationSound? {
        guard let value else { return .funk }
        guard let sound = NotificationSound(rawValue: value), sound != NotificationSound.none else { return nil }
        return sound
    }
}

enum ToastPosition: String, CaseIterable, Identifiable {
    case topCenter = "Top Center"
    case topRight = "Top Right"
    case bottomCenter = "Bottom Center"
    case bottomRight = "Bottom Right"

    var id: String { rawValue }
}

struct NotificationDeliveryPlan: Equatable {
    let showToast: Bool
    let showDesktop: Bool
    let soundName: String?
}

enum NotificationSettings {
    enum Key {
        static let sound = "muxy.notifications.sound"
        static let toastEnabled = "muxy.notifications.toastEnabled"
        static let desktopEnabled = "muxy.notifications.desktopEnabled"
        static let toastPosition = "muxy.notifications.toastPosition"
    }

    enum Default {
        static let sound = NotificationSound.funk
        static let toastEnabled = true
        static let desktopEnabled = false
        static let toastPosition = ToastPosition.topCenter
        static let providerEnabled = true
    }

    static func providerEnabledKey(for providerID: String) -> String {
        "muxy.notifications.provider.\(providerID).enabled"
    }

    static func toastEnabled(defaults: UserDefaults = .standard) -> Bool {
        defaults.bool(forKey: Key.toastEnabled, fallback: Default.toastEnabled)
    }

    static func desktopEnabled(defaults: UserDefaults = .standard) -> Bool {
        defaults.bool(forKey: Key.desktopEnabled, fallback: Default.desktopEnabled)
    }

    static func soundRawValue(defaults: UserDefaults = .standard) -> String {
        defaults.string(forKey: Key.sound) ?? Default.sound.rawValue
    }

    static func sound(defaults: UserDefaults = .standard) -> NotificationSound? {
        NotificationSound(rawValue: soundRawValue(defaults: defaults))
    }

    static func toastPositionRawValue(defaults: UserDefaults = .standard) -> String {
        defaults.string(forKey: Key.toastPosition) ?? Default.toastPosition.rawValue
    }

    static func toastPosition(rawValue: String) -> ToastPosition {
        ToastPosition(rawValue: rawValue) ?? Default.toastPosition
    }

    static func toastPosition(defaults: UserDefaults = .standard) -> ToastPosition {
        toastPosition(rawValue: toastPositionRawValue(defaults: defaults))
    }

    static func providerEnabled(providerID: String, defaults: UserDefaults = .standard) -> Bool {
        defaults.bool(forKey: providerEnabledKey(for: providerID), fallback: Default.providerEnabled)
    }

    static func deliveryPlan(defaults: UserDefaults = .standard) -> NotificationDeliveryPlan {
        let sound = soundRawValue(defaults: defaults)
        return NotificationDeliveryPlan(
            showToast: toastEnabled(defaults: defaults),
            showDesktop: desktopEnabled(defaults: defaults),
            soundName: sound == NotificationSound.none.rawValue ? nil : sound
        )
    }
}

extension UserDefaults {
    func bool(forKey key: String, fallback: Bool) -> Bool {
        object(forKey: key) != nil ? bool(forKey: key) : fallback
    }
}
