import Foundation

enum FilePermissions {
    static let privateFile = 0o600
    static let privateDirectory = 0o700
    static let executable = 0o755
}
