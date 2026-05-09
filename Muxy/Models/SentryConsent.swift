import Foundation

enum SentryConsent: String {
    case allowed
    case denied

    static let storageKey = "muxy.sentry.consent"
}
