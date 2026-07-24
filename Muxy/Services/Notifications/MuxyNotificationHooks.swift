import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "MuxyNotificationHooks")

enum MuxyNotificationHooks {
    static let hookBinaryName = "muxy-hook"

    #if DEBUG
    static let stagingDirectoryName = "hooks-dev"
    #else
    static let stagingDirectoryName = "hooks"
    #endif

    private static let stagedResources = [
        StagedResource(name: "muxy-claude-hook", extension: "sh", executable: true),
        StagedResource(name: "muxy-codex-hook", extension: "sh", executable: true),
        StagedResource(name: "muxy-cursor-hook", extension: "sh", executable: true),
        StagedResource(name: "muxy-droid-hook", extension: "sh", executable: true),
        StagedResource(name: "muxy-grok-hook", extension: "sh", executable: true),
        StagedResource(name: "opencode-muxy-plugin", extension: "js", executable: false),
        StagedResource(name: "muxy-pi-extension", extension: "ts", executable: false),
    ]

    static var stagingDirectoryURL: URL {
        MuxyFileStorage.appSupportDirectory()
            .appendingPathComponent(stagingDirectoryName, isDirectory: true)
    }

    static var hookBinaryPath: String {
        stagingDirectoryURL.appendingPathComponent(hookBinaryName).path
    }

    static var hookScriptPath: String? {
        scriptPath(named: "muxy-claude-hook", extension: "sh")
    }

    static func stagedScriptPath(named name: String, extension ext: String) -> String {
        stagingDirectoryURL.appendingPathComponent("\(name).\(ext)").path
    }

    static func scriptPath(named name: String, extension ext: String) -> String? {
        let path = stagedScriptPath(named: name, extension: ext)
        if FileManager.default.fileExists(atPath: path) {
            return path
        }
        guard stageAll() else { return nil }
        guard FileManager.default.fileExists(atPath: path) else { return nil }
        return path
    }

    @discardableResult
    static func stageAll(
        bundle: Bundle = Bundle.appResources,
        hookBinaryURL: URL? = bundledHookBinaryURL(),
        destinationDirectory: URL? = nil,
        searchDevelopmentDirectory: Bool = true
    ) -> Bool {
        guard let hookBinaryURL else {
            logger.error("Muxy hook bridge binary not found")
            return false
        }

        let destinationDirectory = destinationDirectory ?? stagingDirectoryURL
        do {
            try prepareDestinationDirectory(destinationDirectory)
            _ = try stageFile(
                from: hookBinaryURL,
                to: destinationDirectory.appendingPathComponent(hookBinaryName),
                permissions: FilePermissions.privateExecutable
            )
            for resource in stagedResources {
                guard let sourceURL = sourceScriptURL(
                    named: resource.name,
                    extension: resource.extension,
                    bundle: bundle,
                    searchDevelopmentDirectory: searchDevelopmentDirectory
                )
                else {
                    logger.error("Hook resource \(resource.fileName) not found")
                    return false
                }
                let permissions = resource.executable
                    ? FilePermissions.privateExecutable
                    : FilePermissions.privateFile
                _ = try stageFile(
                    from: sourceURL,
                    to: destinationDirectory.appendingPathComponent(resource.fileName),
                    permissions: permissions
                )
            }
            return true
        } catch {
            logger.error("Failed to stage AI notification hooks: \(error.localizedDescription)")
            return false
        }
    }

    static func findBundledScript(
        _ name: String,
        extension ext: String,
        bundle: Bundle = Bundle.appResources
    ) -> String? {
        let find: (String?) -> URL? = { subdirectory in
            bundle.url(forResource: name, withExtension: ext, subdirectory: subdirectory)
        }
        guard let url = find(nil) ?? find("scripts") else { return nil }
        guard FileManager.default.fileExists(atPath: url.path) else { return nil }
        return url.path
    }

    static func bundledHookBinaryURL() -> URL? {
        guard let executableURL = Bundle.main.executableURL else { return nil }
        let binaryURL = executableURL.deletingLastPathComponent().appendingPathComponent(hookBinaryName)
        guard FileManager.default.isExecutableFile(atPath: binaryURL.path) else { return nil }
        return binaryURL
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
              let developmentPath = findDevelopmentScriptPath("\(name).\(ext)")
        else {
            return nil
        }
        return URL(fileURLWithPath: developmentPath)
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

    @discardableResult
    private static func stageFile(from source: URL, to destination: URL, permissions: Int) throws -> URL {
        if !FileManager.default.contentsEqual(atPath: source.path, andPath: destination.path) {
            try Data(contentsOf: source).write(to: destination, options: .atomic)
        }
        try FileManager.default.setAttributes(
            [.posixPermissions: permissions],
            ofItemAtPath: destination.path
        )
        return destination
    }

    private static func findDevelopmentScriptPath(_ fileName: String) -> String? {
        guard let executableURL = Bundle.main.executableURL else { return nil }
        var directory = executableURL.deletingLastPathComponent()
        for _ in 0 ..< 10 {
            let candidate = directory.appendingPathComponent("scripts/\(fileName)")
            if FileManager.default.fileExists(atPath: candidate.path) {
                return candidate.path
            }
            let parent = directory.deletingLastPathComponent()
            guard parent.path != directory.path else { break }
            directory = parent
        }
        return nil
    }

    private struct StagedResource: Sendable {
        let name: String
        let `extension`: String
        let executable: Bool

        var fileName: String {
            "\(name).\(`extension`)"
        }
    }
}
