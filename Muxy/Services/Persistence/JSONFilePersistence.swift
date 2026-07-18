import Foundation

enum MuxyFileStorage {
    static var isTestProcess: Bool {
        let name = ProcessInfo.processInfo.processName
        return name == "swiftpm-testing-helper" || name == "xctest" || name.hasSuffix("Tests")
    }

    static func fileURL(filename: String) -> URL {
        let dir = appSupportDirectory()
        return dir.appendingPathComponent(filename)
    }

    static func removeFile(named filename: String) {
        try? FileManager.default.removeItem(at: fileURL(filename: filename))
    }

    static func appSupportDirectory(create: Bool = true) -> URL {
        let dir = resolvedAppSupportDirectory()
        guard create else { return dir }
        try? FileManager.default.createDirectory(
            at: dir,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: FilePermissions.privateDirectory]
        )
        return dir
    }

    private static func resolvedAppSupportDirectory() -> URL {
        if isTestProcess {
            if let override = ProcessInfo.processInfo.environment["MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY"],
               !override.isEmpty
            {
                return URL(fileURLWithPath: override, isDirectory: true)
            }
            return FileManager.default.temporaryDirectory
                .appendingPathComponent("MuxyTests-\(ProcessInfo.processInfo.processIdentifier)", isDirectory: true)
        }
        guard let appSupport = FileManager.default.urls(
            for: .applicationSupportDirectory,
            in: .userDomainMask
        ).first
        else {
            fatalError("Application Support directory unavailable")
        }
        return appSupport.appendingPathComponent("Muxy", isDirectory: true)
    }

    static func worktreeRoot(forProjectID projectID: UUID, create: Bool = true) -> URL {
        let dir = appSupportDirectory(create: create)
            .appendingPathComponent("worktree-checkouts", isDirectory: true)
            .appendingPathComponent(projectID.uuidString, isDirectory: true)
        guard create else { return dir }
        try? FileManager.default.createDirectory(
            at: dir,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: FilePermissions.privateDirectory]
        )
        return dir
    }

    static func worktreeDirectory(forProjectID projectID: UUID, name: String) -> URL {
        worktreeRoot(forProjectID: projectID).appendingPathComponent(name, isDirectory: true)
    }
}
