import Foundation

enum BrowserImportSource: String, CaseIterable, Identifiable {
    case chrome

    var id: String { rawValue }

    var displayName: String {
        switch self {
        case .chrome: "Google Chrome"
        }
    }
}

struct ImportableProfile: Identifiable, Hashable {
    let id: String
    let name: String
    let directory: URL
}

enum BrowserImportError: LocalizedError {
    case sourceNotInstalled
    case profileUnavailable
    case keyUnavailable
    case unsupportedEncryption
    case readFailed(String)

    var errorDescription: String? {
        switch self {
        case .sourceNotInstalled: "The selected browser is not installed."
        case .profileUnavailable: "The selected browser profile could not be found."
        case .keyUnavailable: "Could not read the browser's encryption key from the Keychain."
        case .unsupportedEncryption:
            "This browser version stores its key in Apple Passwords and cannot be imported."
        case let .readFailed(reason): "Could not read browser cookies: \(reason)"
        }
    }
}

protocol BrowserImporter {
    var source: BrowserImportSource { get }
    func isInstalled() -> Bool
    func availableProfiles() throws -> [ImportableProfile]
    func importCookies(from profile: ImportableProfile) throws -> [HTTPCookie]
}
