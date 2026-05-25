import Foundation

struct PiProvider: AIProviderIntegration, AIUsageProvider {
    let id = "pi"
    let displayName = "Pi"
    let socketTypeKey = "pi"
    let iconName = "pi"
    let executableNames = ["pi"]
    let hookScriptName = "muxy-pi-extension"
    let hookScriptExtension = "ts"

    private static let destinationFileName = "muxy-notify.ts"
    private static let bundleResourceName = "muxy-pi-extension"
    private static let bundleResourceExtension = "ts"

    private let homeDirectory: String
    private let pathEnvironment: String
    private let resourceURL: @Sendable (String, String) -> URL?

    init(
        homeDirectory: String = NSHomeDirectory(),
        pathEnvironment: String = ProcessInfo.processInfo.environment["PATH"] ?? "",
        resourceURL: @escaping @Sendable (String, String) -> URL? = { name, ext in
            Bundle.appResources.url(forResource: name, withExtension: ext)
        }
    ) {
        self.homeDirectory = homeDirectory
        self.pathEnvironment = pathEnvironment
        self.resourceURL = resourceURL
    }

    private var extensionsDir: String { homeDirectory + "/.pi/agent/extensions" }
    private var destinationPath: String { extensionsDir + "/" + Self.destinationFileName }
    private var settingsPath: String { homeDirectory + "/.pi/agent/settings.json" }

    func isToolInstalled() -> Bool {
        let paths = [
            "\(homeDirectory)/.local/bin/pi",
            "/usr/local/bin/pi",
            "/opt/homebrew/bin/pi",
        ] + pathEnvironment
            .split(separator: ":", omittingEmptySubsequences: true)
            .map { "\($0)/pi" }

        return paths.contains { FileManager.default.isExecutableFile(atPath: $0) }
    }

    func install(hookScriptPath: String) throws {
        guard let sourceURL = resourceURL(Self.bundleResourceName, Self.bundleResourceExtension) else {
            throw PiProviderError.bundleResourceNotFound
        }

        let sourceData = try Data(contentsOf: sourceURL)

        if FileManager.default.fileExists(atPath: destinationPath),
           let existingData = try? Data(contentsOf: URL(fileURLWithPath: destinationPath)),
           existingData == sourceData
        {
            return
        }

        try FileManager.default.createDirectory(
            atPath: extensionsDir,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: FilePermissions.privateDirectory]
        )

        let destURL = URL(fileURLWithPath: destinationPath)
        if FileManager.default.fileExists(atPath: destinationPath) {
            try FileManager.default.removeItem(at: destURL)
        }

        try sourceData.write(to: destURL, options: .atomic)

        try registerExtensionInSettings()
    }

    func uninstall() throws {
        guard FileManager.default.fileExists(atPath: destinationPath) else { return }
        try FileManager.default.removeItem(atPath: destinationPath)
        try unregisterExtensionFromSettings()
    }

    private func registerExtensionInSettings() throws {
        guard FileManager.default.fileExists(atPath: settingsPath) else { return }
        let url = URL(fileURLWithPath: settingsPath)
        let data = try Data(contentsOf: url)
        guard var json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { return }

        var extensions = json["extensions"] as? [String] ?? []
        let extensionPath = destinationPath

        if !extensions.contains(extensionPath) {
            extensions.append(extensionPath)
            json["extensions"] = extensions
            let updatedData = try JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])

            let backupPath = settingsPath + ".muxy-backup"
            try? FileManager.default.removeItem(atPath: backupPath)
            try FileManager.default.copyItem(atPath: settingsPath, toPath: backupPath)

            try updatedData.write(to: url, options: .atomic)
            try FileManager.default.setAttributes(
                [.posixPermissions: FilePermissions.privateFile],
                ofItemAtPath: settingsPath
            )
        }
    }

    private func unregisterExtensionFromSettings() throws {
        guard FileManager.default.fileExists(atPath: settingsPath) else { return }
        let url = URL(fileURLWithPath: settingsPath)
        let data = try Data(contentsOf: url)
        guard var json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { return }

        guard var extensions = json["extensions"] as? [String] else { return }
        extensions.removeAll { $0 == destinationPath }

        if extensions.isEmpty {
            json.removeValue(forKey: "extensions")
        } else {
            json["extensions"] = extensions
        }

        let updatedData = try JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])

        let backupPath = settingsPath + ".muxy-backup"
        try? FileManager.default.removeItem(atPath: backupPath)
        try FileManager.default.copyItem(atPath: settingsPath, toPath: backupPath)

        try updatedData.write(to: url, options: .atomic)
        try FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.privateFile],
            ofItemAtPath: settingsPath
        )
    }
}

enum PiProviderError: LocalizedError {
    case bundleResourceNotFound

    var errorDescription: String? {
        switch self {
        case .bundleResourceNotFound:
            "Pi extension file (muxy-pi-extension.ts) not found in app bundle"
        }
    }
}
