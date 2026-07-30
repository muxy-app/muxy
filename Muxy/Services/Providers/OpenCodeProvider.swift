import Foundation

struct OpenCodeProvider: AIProviderIntegration, AIAgentLaunchProvider, AIProviderDiscoveryIntegration {
    let id = "opencode"
    let displayName = "OpenCode"
    let socketTypeKey = "opencode"
    let iconName = "opencode"
    let executableNames = ["opencode"]
    let hookScriptName = "opencode-muxy-plugin"
    let hookScriptExtension = "js"

    var agentLaunchConfiguration: AIAgentLaunchConfiguration {
        AIAgentLaunchConfiguration(
            executable: "opencode",
            headlessArguments: ["run", "--pure"],
            environment: ["OPENCODE_PERMISSION": #"{"*":"deny"}"#]
        )
    }

    private static let pluginFileName = "muxy-notify.js"
    private let homeDirectory: String
    private let pathEnvironment: @Sendable () -> String

    init(
        homeDirectory: String = NSHomeDirectory(),
        pathEnvironment: @escaping @Sendable () -> String = { LoginShellPath.current }
    ) {
        self.homeDirectory = homeDirectory
        self.pathEnvironment = pathEnvironment
    }

    init(homeDirectory: String = NSHomeDirectory(), pathEnvironment: String) {
        self.init(homeDirectory: homeDirectory, pathEnvironment: { pathEnvironment })
    }

    private var pluginsDirectory: String { homeDirectory + "/.config/opencode/plugins" }
    private var pluginPath: String { pluginsDirectory + "/" + Self.pluginFileName }
    private var legacyPluginPath: String { homeDirectory + "/.opencode/plugins/" + Self.pluginFileName }

    var discoveryArguments: [String] { ["debug", "info"] }
    var discoveryWorkingDirectory: String { homeDirectory }

    func isToolInstalled() -> Bool {
        agentCLIExecutablePath() != nil
    }

    func agentCLIExecutablePath() -> String? {
        ProviderExecutableLocator.executablePath(
            names: [agentLaunchConfiguration.executable],
            homeDirectory: homeDirectory,
            pathEnvironment: pathEnvironment(),
            includeSystemWide: homeDirectory == NSHomeDirectory(),
            homeRelativeBins: [".opencode/bin", ".local/bin"]
        )
    }

    func discoveryDetails(from output: String) -> ProviderDiscoveryDetails {
        let lines = output.split(whereSeparator: \.isNewline).map {
            $0.trimmingCharacters(in: .whitespacesAndNewlines)
        }
        let version = lines.first(where: { $0.hasPrefix("opencode version:") })?
            .dropFirst("opencode version:".count)
            .trimmingCharacters(in: .whitespacesAndNewlines)
        let pluginLines = lines
            .drop { $0 != "plugins:" }
            .dropFirst()
            .prefix { $0.hasPrefix("- ") }
        let plugins = Set(pluginLines.compactMap { line -> String? in
            guard line.hasPrefix("- ") else { return nil }
            return String(line.dropFirst(2))
        })
        let currentURL = URL(fileURLWithPath: pluginPath).absoluteString
        if plugins.contains(currentURL) || plugins.contains(pluginPath) {
            return ProviderDiscoveryDetails(version: version, state: .ready)
        }
        let legacyURL = URL(fileURLWithPath: legacyPluginPath).absoluteString
        if plugins.contains(legacyURL) || plugins.contains(legacyPluginPath) {
            return ProviderDiscoveryDetails(
                version: version,
                state: .warning("Legacy Muxy plugin discovered")
            )
        }
        return ProviderDiscoveryDetails(
            version: version,
            state: .warning("Muxy plugin not discovered by OpenCode")
        )
    }

    func isHookInstalled() -> Bool {
        FileManager.default.fileExists(atPath: pluginPath)
    }

    func hasManagedState() -> Bool {
        isHookInstalled() || hasObsoletePlugin
    }

    var configPaths: [String] { [pluginPath] }

    var obsoleteConfigPaths: [String] { [legacyPluginPath] }

    func verify(hookScriptPath: String) -> HookVerification {
        guard FileManager.default.fileExists(atPath: pluginPath) else { return .needsRepair }
        guard FileManager.default.contentsEqual(atPath: hookScriptPath, andPath: pluginPath) else {
            return .needsRepair
        }
        guard installedPluginHasPrivatePermissions() else { return .needsRepair }
        guard !hasObsoletePlugin else { return .needsRepair }
        return .satisfied
    }

    func install(hookScriptPath: String) throws {
        let sourceData = try Data(contentsOf: URL(fileURLWithPath: hookScriptPath))

        try FileManager.default.createDirectory(
            atPath: pluginsDirectory,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: FilePermissions.privateDirectory]
        )
        let pluginURL = URL(fileURLWithPath: pluginPath)
        let existingData = try? Data(contentsOf: pluginURL)
        let contentChanged = existingData != sourceData
        if contentChanged {
            try sourceData.write(to: pluginURL, options: .atomic)
        }
        try FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.privateFile],
            ofItemAtPath: pluginPath
        )
        if contentChanged {
            HookConfigWriteLedger.shared.recordWrite(path: pluginPath, contents: sourceData)
        }
        try removeLegacyPlugin()
    }

    func uninstall() throws {
        if FileManager.default.fileExists(atPath: pluginPath) {
            try FileManager.default.removeItem(atPath: pluginPath)
        }
        try removeLegacyPlugin()
    }

    private func installedPluginHasPrivatePermissions() -> Bool {
        guard let attributes = try? FileManager.default.attributesOfItem(atPath: pluginPath),
              let permissions = attributes[.posixPermissions] as? NSNumber
        else { return false }
        return permissions.intValue == FilePermissions.privateFile
    }

    private var hasObsoletePlugin: Bool {
        obsoleteConfigPaths.contains { FileManager.default.fileExists(atPath: $0) }
    }

    private func removeLegacyPlugin() throws {
        for path in obsoleteConfigPaths where FileManager.default.fileExists(atPath: path) {
            try FileManager.default.removeItem(atPath: path)
        }
    }
}
