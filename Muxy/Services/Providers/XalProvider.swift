import Foundation

struct XalProvider: AIProviderIntegration, AIAgentLaunchProvider {
    let id = "xal"
    let displayName = "Xal"
    let socketTypeKey = "xal"
    let iconName = "xal"
    let executableNames = ["xal"]
    let hookScriptName = "muxy-xal-plugin"
    let hookScriptExtension = "ts"

    var agentLaunchConfiguration: AIAgentLaunchConfiguration {
        AIAgentLaunchConfiguration(
            executable: "xal",
            headlessArguments: ["run", "--format", "text"]
        )
    }

    private static let pluginDirectoryName = "muxy-notify"
    private static let pluginFileName = "plugin.ts"
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

    private var agentHome: String { homeDirectory + "/.xal" }
    private var pluginDirectory: String { agentHome + "/plugins/" + Self.pluginDirectoryName }
    private var pluginPath: String { pluginDirectory + "/" + Self.pluginFileName }
    private var configurationPath: String { agentHome + "/config.json" }

    var configPaths: [String] { [pluginPath, configurationPath] }

    func isToolInstalled() -> Bool {
        agentCLIExecutablePath() != nil
    }

    func agentCLIExecutablePath() -> String? {
        ProviderExecutableLocator.executablePath(
            names: [agentLaunchConfiguration.executable],
            homeDirectory: homeDirectory,
            pathEnvironment: pathEnvironment(),
            includeSystemWide: homeDirectory == NSHomeDirectory(),
            homeRelativeBins: [".local/bin"]
        )
    }

    func isHookInstalled() -> Bool {
        FileManager.default.fileExists(atPath: pluginPath)
    }

    func hasManagedState() -> Bool {
        isHookInstalled() || isRegisteredInConfiguration()
    }

    func verify(hookScriptPath: String) -> HookVerification {
        guard FileManager.default.fileExists(atPath: pluginPath) else { return .needsRepair }
        guard FileManager.default.contentsEqual(atPath: hookScriptPath, andPath: pluginPath) else {
            return .needsRepair
        }
        guard installedPluginHasPrivatePermissions() else { return .needsRepair }
        guard isRegisteredInConfiguration() else { return .needsRepair }
        return .satisfied
    }

    func install(hookScriptPath: String) throws {
        let sourceURL = URL(fileURLWithPath: hookScriptPath)
        guard FileManager.default.fileExists(atPath: sourceURL.path) else {
            throw XalProviderError.hookResourceNotFound
        }
        let sourceData = try Data(contentsOf: sourceURL)

        try FileManager.default.createDirectory(
            atPath: pluginDirectory,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: FilePermissions.privateDirectory]
        )
        let pluginURL = URL(fileURLWithPath: pluginPath)
        let existingData = try? Data(contentsOf: pluginURL)
        if existingData != sourceData {
            try sourceData.write(to: pluginURL, options: .atomic)
            HookConfigWriteLedger.shared.recordWrite(path: pluginPath, contents: sourceData)
        }
        try FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.privateFile],
            ofItemAtPath: pluginPath
        )
        try registerPluginInConfiguration()
    }

    func uninstall() throws {
        if FileManager.default.fileExists(atPath: pluginDirectory) {
            try FileManager.default.removeItem(atPath: pluginDirectory)
        }
        try unregisterPluginFromConfiguration()
    }

    private func registerPluginInConfiguration() throws {
        var configuration = try readConfiguration()
        var plugins = configuration["plugins"] as? [String] ?? []
        guard !plugins.contains(pluginDirectory) else { return }
        plugins.append(pluginDirectory)
        configuration["plugins"] = plugins
        try HookConfigWriter.write(configuration, to: configurationPath)
    }

    private func unregisterPluginFromConfiguration() throws {
        guard FileManager.default.fileExists(atPath: configurationPath) else { return }
        var configuration = try readConfiguration()
        guard var plugins = configuration["plugins"] as? [String],
              plugins.contains(pluginDirectory)
        else { return }
        plugins.removeAll { $0 == pluginDirectory }
        if plugins.isEmpty {
            configuration.removeValue(forKey: "plugins")
        } else {
            configuration["plugins"] = plugins
        }
        try HookConfigWriter.write(configuration, to: configurationPath)
    }

    private func isRegisteredInConfiguration() -> Bool {
        guard let configuration = try? readConfiguration(),
              let plugins = configuration["plugins"] as? [String]
        else { return false }
        return plugins.contains(pluginDirectory)
    }

    private func readConfiguration() throws -> [String: Any] {
        guard FileManager.default.fileExists(atPath: configurationPath) else { return [:] }
        let data = try Data(contentsOf: URL(fileURLWithPath: configurationPath))
        guard !data.isEmpty else { return [:] }
        guard let configuration = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw XalProviderError.malformedConfiguration(configurationPath)
        }
        return configuration
    }

    private func installedPluginHasPrivatePermissions() -> Bool {
        guard let attributes = try? FileManager.default.attributesOfItem(atPath: pluginPath),
              let permissions = attributes[.posixPermissions] as? NSNumber
        else { return false }
        return permissions.intValue == FilePermissions.privateFile
    }
}

enum XalProviderError: LocalizedError, Equatable {
    case hookResourceNotFound
    case malformedConfiguration(String)

    var errorDescription: String? {
        switch self {
        case .hookResourceNotFound:
            "Xal plugin file (muxy-xal-plugin.ts) not found at the staged hook path"
        case let .malformedConfiguration(path):
            "Xal configuration at \(path) is not a JSON object"
        }
    }
}
