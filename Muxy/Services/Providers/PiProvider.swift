import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "PiProvider")

struct PiProvider: AIProviderIntegration, AIAgentLaunchProvider {
    let id = "pi"
    let displayName = "Pi"
    let socketTypeKey = "pi"
    let iconName = "pi"
    let executableNames = ["pi"]
    let hookScriptName = "muxy-pi-extension"
    let hookScriptExtension = "ts"

    var agentLaunchConfiguration: AIAgentLaunchConfiguration {
        AIAgentLaunchConfiguration(
            executable: "pi",
            headlessArguments: ["--print", "--no-session", "--no-tools"]
        )
    }

    private static let destinationFileName = "muxy-notify.ts"
    private let homeDirectory: String
    private let pathEnvironment: @Sendable () -> String

    init(
        homeDirectory: String = NSHomeDirectory(),
        pathEnvironment: @escaping @Sendable () -> String = { LoginShellPath.current }
    ) {
        self.homeDirectory = homeDirectory
        self.pathEnvironment = pathEnvironment
    }

    init(
        homeDirectory: String = NSHomeDirectory(),
        pathEnvironment: String
    ) {
        self.init(homeDirectory: homeDirectory, pathEnvironment: { pathEnvironment })
    }

    private var extensionsDir: String { homeDirectory + "/.pi/agent/extensions" }
    private var destinationPath: String { extensionsDir + "/" + Self.destinationFileName }
    private var settingsPath: String { homeDirectory + "/.pi/agent/settings.json" }

    func isToolInstalled() -> Bool {
        agentCLIExecutablePath() != nil
    }

    func agentCLIExecutablePath() -> String? {
        ProviderExecutableLocator.executablePath(
            names: [agentLaunchConfiguration.executable],
            homeDirectory: homeDirectory,
            pathEnvironment: pathEnvironment(),
            includeSystemWide: homeDirectory == NSHomeDirectory()
        )
    }

    func isHookInstalled() -> Bool {
        FileManager.default.fileExists(atPath: destinationPath)
    }

    func hasManagedState() -> Bool {
        isHookInstalled() || isRegisteredInSettings()
    }

    var configPaths: [String] { [destinationPath, settingsPath] }

    func verify(hookScriptPath: String) -> HookVerification {
        guard FileManager.default.fileExists(atPath: destinationPath) else { return .needsRepair }
        guard FileManager.default.contentsEqual(atPath: hookScriptPath, andPath: destinationPath) else {
            return .needsRepair
        }
        guard installedExtensionHasPrivatePermissions() else { return .needsRepair }
        guard !isRegisteredInSettings() else { return .needsRepair }
        return .satisfied
    }

    func install(hookScriptPath: String) throws {
        let sourceURL = URL(fileURLWithPath: hookScriptPath)
        guard FileManager.default.fileExists(atPath: sourceURL.path) else { throw PiProviderError.hookResourceNotFound }
        let sourceData = try Data(contentsOf: sourceURL)

        if FileManager.default.fileExists(atPath: destinationPath),
           let existingData = try? Data(contentsOf: URL(fileURLWithPath: destinationPath)),
           existingData == sourceData
        {
            try setPrivatePermissionsOnInstalledExtension()
            try unregisterExtensionFromSettings()
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
        try setPrivatePermissionsOnInstalledExtension()
        HookConfigWriteLedger.shared.recordWrite(path: destinationPath, contents: sourceData)
        try unregisterExtensionFromSettings()
    }

    func uninstall() throws {
        if FileManager.default.fileExists(atPath: destinationPath) {
            try FileManager.default.removeItem(atPath: destinationPath)
        }
        try unregisterExtensionFromSettings()
    }

    private func unregisterExtensionFromSettings() throws {
        guard FileManager.default.fileExists(atPath: settingsPath) else {
            logger.info("No settings.json found, nothing to unregister")
            return
        }
        let url = URL(fileURLWithPath: settingsPath)
        let data = try Data(contentsOf: url)
        guard var json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            logger.warning("Cannot parse settings.json, skipping unregister")
            return
        }

        guard var extensions = json["extensions"] as? [String] else { return }
        guard extensions.contains(destinationPath) else { return }
        extensions.removeAll { $0 == destinationPath }

        if extensions.isEmpty {
            json.removeValue(forKey: "extensions")
        } else {
            json["extensions"] = extensions
        }

        try HookConfigWriter.write(json, to: settingsPath)
    }

    private func isRegisteredInSettings() -> Bool {
        guard let data = try? Data(contentsOf: URL(fileURLWithPath: settingsPath)),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let extensions = json["extensions"] as? [String]
        else { return false }
        return extensions.contains(destinationPath)
    }

    private func setPrivatePermissionsOnInstalledExtension() throws {
        try FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.privateFile],
            ofItemAtPath: destinationPath
        )
    }

    private func installedExtensionHasPrivatePermissions() -> Bool {
        guard let attributes = try? FileManager.default.attributesOfItem(atPath: destinationPath),
              let permissions = attributes[.posixPermissions] as? NSNumber
        else { return false }
        return permissions.intValue == FilePermissions.privateFile
    }
}

enum PiProviderError: LocalizedError, Equatable {
    case hookResourceNotFound

    var errorDescription: String? {
        switch self {
        case .hookResourceNotFound:
            "Pi extension file (muxy-pi-extension.ts) not found at the staged hook path"
        }
    }
}
