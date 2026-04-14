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
}

enum ToastPosition: String, CaseIterable, Identifiable {
    case topCenter = "Top Center"
    case topRight = "Top Right"
    case bottomCenter = "Bottom Center"
    case bottomRight = "Bottom Right"

    var id: String { rawValue }
}

enum AutoClearDuration: String, CaseIterable, Identifiable {
    case off = "Off"
    case fiveSeconds = "5 seconds"
    case tenSeconds = "10 seconds"
    case thirtySeconds = "30 seconds"
    case oneMinute = "1 minute"

    var id: String { rawValue }

    var seconds: Double? {
        switch self {
        case .off: nil
        case .fiveSeconds: 5
        case .tenSeconds: 10
        case .thirtySeconds: 30
        case .oneMinute: 60
        }
    }
}
