import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "PiProvider")

struct PiProvider: AIProviderIntegration {
    let id = "pi"
    let displayName = "Pi"
    let socketTypeKey = "pi"
    let iconName = "pi"
    let executableNames = ["pi"]
    let hookScriptName = "muxy-pi-extension"
    let hookScriptExtension = "ts"

    private static let legacyDestinationFileName = "muxy-notify.ts"
    private static let bundleExtensionName = "muxy-pi-extension"
    private static let bundleExtensionExtension = "ts"
    private static let bundleWrapperName = "muxy-pi-wrapper"
    private static let bundleWrapperExtension = "sh"

    private let homeDirectory: String
    private let pathEnvironment: @Sendable () -> String
    private let appSupportDirectory: URL
    private let resourceURL: @Sendable (String, String) -> URL?

    init(
        homeDirectory: String = NSHomeDirectory(),
        pathEnvironment: @escaping @Sendable () -> String = { LoginShellPath.current },
        appSupportDirectory: URL = MuxyFileStorage.appSupportDirectory(),
        resourceURL: @escaping @Sendable (String, String) -> URL? = { name, ext in
            let bundle = Bundle.appResources
            return bundle.url(forResource: name, withExtension: ext, subdirectory: nil)
                ?? bundle.url(forResource: name, withExtension: ext, subdirectory: "scripts")
        }
    ) {
        self.homeDirectory = homeDirectory
        self.pathEnvironment = pathEnvironment
        self.appSupportDirectory = appSupportDirectory
        self.resourceURL = resourceURL
    }

    init(
        homeDirectory: String = NSHomeDirectory(),
        pathEnvironment: String,
        appSupportDirectory: URL = MuxyFileStorage.appSupportDirectory(),
        resourceURL: @escaping @Sendable (String, String) -> URL? = { name, ext in
            let bundle = Bundle.appResources
            return bundle.url(forResource: name, withExtension: ext, subdirectory: nil)
                ?? bundle.url(forResource: name, withExtension: ext, subdirectory: "scripts")
        }
    ) {
        self.init(
            homeDirectory: homeDirectory,
            pathEnvironment: { pathEnvironment },
            appSupportDirectory: appSupportDirectory,
            resourceURL: resourceURL
        )
    }

    private var legacyExtensionsDir: String { homeDirectory + "/.pi/agent/extensions" }
    private var legacyDestinationPath: String { legacyExtensionsDir + "/" + Self.legacyDestinationFileName }
    private var legacySettingsPath: String { homeDirectory + "/.pi/agent/settings.json" }
    private var binDirectory: URL { MuxyAgentBin.directory(appSupportDirectory: appSupportDirectory) }
    private var wrapperURL: URL { MuxyAgentBin.wrapperURL(appSupportDirectory: appSupportDirectory) }
    private var extensionURL: URL { MuxyAgentBin.extensionURL(appSupportDirectory: appSupportDirectory) }

    func isToolInstalled() -> Bool {
        let muxyBin = binDirectory.path
        let paths = [
            "\(homeDirectory)/.local/bin/pi",
            "/usr/local/bin/pi",
            "/opt/homebrew/bin/pi",
        ] + pathEnvironment()
            .split(separator: ":", omittingEmptySubsequences: true)
            .map { "\($0)/pi" }
            .filter { !$0.hasPrefix(muxyBin + "/") }

        return paths.contains { FileManager.default.isExecutableFile(atPath: $0) }
    }

    func install(hookScriptPath: String) throws {
        guard let sourceExtensionURL = resourceURL(Self.bundleExtensionName, Self.bundleExtensionExtension) else {
            throw PiProviderError.bundleResourceNotFound
        }
        guard let sourceWrapperURL = resourceURL(Self.bundleWrapperName, Self.bundleWrapperExtension) else {
            throw PiProviderError.bundleWrapperNotFound
        }

        let extensionData = try Data(contentsOf: sourceExtensionURL)
        let wrapperData = try Data(contentsOf: sourceWrapperURL)

        if isCurrentInstall(extensionData: extensionData, wrapperData: wrapperData) {
            try removeLegacyInstall()
            return
        }

        try FileManager.default.createDirectory(
            at: binDirectory,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: FilePermissions.privateDirectory]
        )

        try writeExecutableFile(extensionData, to: extensionURL)
        try writeExecutableFile(wrapperData, to: wrapperURL)

        try removeLegacyInstall()
    }

    func uninstall() throws {
        try? FileManager.default.removeItem(at: wrapperURL)
        try? FileManager.default.removeItem(at: extensionURL)
        try removeLegacyInstall()
    }

    private func isCurrentInstall(extensionData: Data, wrapperData: Data) -> Bool {
        guard FileManager.default.fileExists(atPath: wrapperURL.path),
              FileManager.default.fileExists(atPath: extensionURL.path),
              let installedExtension = try? Data(contentsOf: extensionURL),
              let installedWrapper = try? Data(contentsOf: wrapperURL)
        else {
            return false
        }
        return installedExtension == extensionData && installedWrapper == wrapperData
    }

    private func writeExecutableFile(_ data: Data, to url: URL) throws {
        if FileManager.default.fileExists(atPath: url.path) {
            try FileManager.default.removeItem(at: url)
        }
        try data.write(to: url, options: .atomic)
        try FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.executable],
            ofItemAtPath: url.path
        )
    }

    private func removeLegacyInstall() throws {
        if FileManager.default.fileExists(atPath: legacyDestinationPath) {
            try FileManager.default.removeItem(atPath: legacyDestinationPath)
        }
        try unregisterExtensionFromLegacySettings()
    }

    private func unregisterExtensionFromLegacySettings() throws {
        guard FileManager.default.fileExists(atPath: legacySettingsPath) else { return }
        let url = URL(fileURLWithPath: legacySettingsPath)
        let data = try Data(contentsOf: url)
        guard var json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            logger.warning("Cannot parse settings.json, skipping unregister")
            return
        }

        guard var extensions = json["extensions"] as? [String] else { return }
        let hadLegacyRegistration = extensions.contains(legacyDestinationPath)
        extensions.removeAll { $0 == legacyDestinationPath }
        guard hadLegacyRegistration else { return }

        if extensions.isEmpty {
            json.removeValue(forKey: "extensions")
        } else {
            json["extensions"] = extensions
        }

        let updatedData = try JSONSerialization.data(withJSONObject: json, options: [.prettyPrinted, .sortedKeys])
        try updatedData.write(to: url, options: .atomic)
        try FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.privateFile],
            ofItemAtPath: legacySettingsPath
        )
    }
}

enum PiProviderError: LocalizedError, Equatable {
    case bundleResourceNotFound
    case bundleWrapperNotFound

    var errorDescription: String? {
        switch self {
        case .bundleResourceNotFound:
            "Pi extension file (muxy-pi-extension.ts) not found in app bundle"
        case .bundleWrapperNotFound:
            "Pi wrapper script (muxy-pi-wrapper.sh) not found in app bundle"
        }
    }
}
