import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "MuxyNotificationHooks")

enum MuxyNotificationHooks {
    private static let hookScriptName = "muxy-claude-hook"
    private static let sharedShellRuntimeName = "muxy-agent-hook"
    private static let scriptsDirectoryName = "hooks"

    static var hookScriptPath: String? {
        sourceScriptURL(
            named: hookScriptName,
            extension: "sh",
            bundle: Bundle.appResources,
            searchDevelopmentDirectory: true
        )?.path
    }

    static func scriptPath(named name: String, extension ext: String) -> String? {
        stageScript(
            named: name,
            extension: ext,
            destinationDirectory: MuxyFileStorage.appSupportDirectory()
                .appendingPathComponent(scriptsDirectoryName, isDirectory: true),
            searchDevelopmentDirectory: true
        )
    }

    static func findBundledScript(_ name: String, extension ext: String, bundle: Bundle = Bundle.appResources) -> String? {
        let find: (String?) -> URL? = { sub in
            bundle.url(forResource: name, withExtension: ext, subdirectory: sub)
        }

        guard let url = find(nil) ?? find("scripts") else {
            return nil
        }

        let path = url.path
        guard FileManager.default.fileExists(atPath: path) else { return nil }

        return path
    }

    static func stageScript(
        named name: String,
        extension ext: String,
        bundle: Bundle = Bundle.appResources,
        destinationDirectory: URL,
        searchDevelopmentDirectory: Bool = false
    ) -> String? {
        guard let sourceURL = sourceScriptURL(
            named: name,
            extension: ext,
            bundle: bundle,
            searchDevelopmentDirectory: searchDevelopmentDirectory
        )
        else {
            return nil
        }

        do {
            try prepareDestinationDirectory(destinationDirectory)
            if ext == "sh", name != sharedShellRuntimeName {
                guard let runtimeURL = sourceScriptURL(
                    named: sharedShellRuntimeName,
                    extension: "sh",
                    bundle: bundle,
                    searchDevelopmentDirectory: searchDevelopmentDirectory
                )
                else {
                    logger.error("Shared shell hook runtime not found")
                    return nil
                }
                _ = try stageFile(
                    from: runtimeURL,
                    to: destinationDirectory,
                    permissions: FilePermissions.privateExecutable
                )
            }
            let permissions = ext == "sh" ? FilePermissions.privateExecutable : FilePermissions.privateFile
            return try stageFile(from: sourceURL, to: destinationDirectory, permissions: permissions).path
        } catch {
            logger.error("Failed to stage \(name).\(ext): \(error.localizedDescription)")
            return nil
        }
    }

    private static func sourceScriptURL(
        named name: String,
        extension ext: String,
        bundle: Bundle,
        searchDevelopmentDirectory: Bool
    ) -> URL? {
        if let bundled = findBundledScript(name, extension: ext, bundle: bundle) {
            return URL(fileURLWithPath: bundled)
        }
        guard searchDevelopmentDirectory,
              let devPath = findDevScriptPath(name + "." + ext)
        else {
            return nil
        }
        return URL(fileURLWithPath: devPath)
    }

    private static func prepareDestinationDirectory(_ directory: URL) throws {
        try FileManager.default.createDirectory(
            at: directory,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: FilePermissions.privateDirectory]
        )
        try FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.privateDirectory],
            ofItemAtPath: directory.path
        )
    }

    private static func stageFile(from source: URL, to directory: URL, permissions: Int) throws -> URL {
        let destination = directory.appendingPathComponent(source.lastPathComponent)
        let sourceData = try Data(contentsOf: source)
        let existingData = try? Data(contentsOf: destination)
        if existingData != sourceData {
            try sourceData.write(to: destination, options: .atomic)
        }
        try FileManager.default.setAttributes(
            [.posixPermissions: permissions],
            ofItemAtPath: destination.path
        )
        return destination
    }

    private static func findDevScriptPath(_ fileName: String) -> String? {
        guard let execURL = Bundle.main.executableURL else { return nil }
        var dir = execURL.deletingLastPathComponent()
        for _ in 0 ..< 10 {
            let candidate = dir.appendingPathComponent("scripts/\(fileName)")
            if FileManager.default.fileExists(atPath: candidate.path) {
                return candidate.path
            }
            let parent = dir.deletingLastPathComponent()
            guard parent.path != dir.path else { break }
            dir = parent
        }
        return nil
    }
}
