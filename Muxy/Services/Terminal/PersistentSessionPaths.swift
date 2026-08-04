import Darwin
import Foundation

enum PersistentSessionPathError: Error, Equatable {
    case pathTooLong
    case insecureFallbackDirectory
    case directoryUnavailable
}

enum PersistentSessionPaths {
    static let sunPathCapacity = 104
    static let socketFileName = "control.sock"
    static let binaryName = "muxy-session"

    static func fits(_ path: String) -> Bool {
        path.utf8.count < sunPathCapacity
    }

    static func directoryName(isDevelopment: Bool = AppEnvironment.isDevelopment) -> String {
        isDevelopment ? "sessions-dev" : "sessions"
    }

    static func preferredSocketPath(
        appSupportDirectory: URL,
        isDevelopment: Bool = AppEnvironment.isDevelopment
    ) -> String {
        appSupportDirectory
            .appendingPathComponent(directoryName(isDevelopment: isDevelopment), isDirectory: true)
            .appendingPathComponent(socketFileName)
            .path
    }

    static func fallbackSocketPath(
        userID: uid_t,
        root: String = "/tmp",
        isDevelopment: Bool = AppEnvironment.isDevelopment
    ) -> String {
        let prefix = isDevelopment ? "muxy-dev" : "muxy"
        return "\(root)/\(prefix)-\(userID)/\(socketFileName)"
    }

    static func resolveSocketPath(
        appSupportDirectory: URL = MuxyFileStorage.appSupportDirectory(),
        userID: uid_t = getuid(),
        fallbackRoot: String = "/tmp",
        fileManager: FileManager = .default,
        isDevelopment: Bool = AppEnvironment.isDevelopment
    ) throws -> String {
        let preferred = preferredSocketPath(appSupportDirectory: appSupportDirectory, isDevelopment: isDevelopment)
        if fits(preferred) {
            try prepareDirectory(at: URL(fileURLWithPath: preferred).deletingLastPathComponent(), fileManager: fileManager)
            return preferred
        }

        let fallback = fallbackSocketPath(userID: userID, root: fallbackRoot, isDevelopment: isDevelopment)
        guard fits(fallback) else { throw PersistentSessionPathError.pathTooLong }
        let directory = URL(fileURLWithPath: fallback).deletingLastPathComponent()
        try prepareDirectory(at: directory, fileManager: fileManager)
        try secureFallbackDirectory(at: directory, userID: userID, fileManager: fileManager)
        return fallback
    }

    static func binaryURL(executableURL: URL? = Bundle.main.executableURL, fileManager: FileManager = .default) -> URL? {
        guard let executableURL else { return nil }
        let sibling = executableURL.deletingLastPathComponent().appendingPathComponent(binaryName)
        guard fileManager.isExecutableFile(atPath: sibling.path) else { return nil }
        return sibling
    }

    private static func prepareDirectory(at url: URL, fileManager: FileManager) throws {
        do {
            try fileManager.createDirectory(
                at: url,
                withIntermediateDirectories: true,
                attributes: [.posixPermissions: FilePermissions.privateDirectory]
            )
        } catch {
            var isDirectory: ObjCBool = false
            guard fileManager.fileExists(atPath: url.path, isDirectory: &isDirectory), isDirectory.boolValue else {
                throw PersistentSessionPathError.directoryUnavailable
            }
        }
    }

    private static func secureFallbackDirectory(at url: URL, userID: uid_t, fileManager: FileManager) throws {
        let attributes = try fileManager.attributesOfItem(atPath: url.path)
        guard let owner = attributes[.ownerAccountID] as? NSNumber, uid_t(owner.uint32Value) == userID else {
            throw PersistentSessionPathError.insecureFallbackDirectory
        }
        guard let permissions = attributes[.posixPermissions] as? NSNumber else {
            throw PersistentSessionPathError.insecureFallbackDirectory
        }
        guard permissions.int16Value & 0o077 != 0 else { return }
        try fileManager.setAttributes(
            [.posixPermissions: FilePermissions.privateDirectory],
            ofItemAtPath: url.path
        )
    }
}
