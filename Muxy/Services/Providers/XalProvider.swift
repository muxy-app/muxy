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
    private let configurationWriter: ([String: Any], String) throws -> Void
    private let pluginDirectoryRemover: (String) throws -> Void

    init(
        homeDirectory: String = NSHomeDirectory(),
        pathEnvironment: @escaping @Sendable () -> String = { LoginShellPath.current },
        configurationWriter: @escaping ([String: Any], String) throws -> Void = {
            try HookConfigWriter.write($0, to: $1)
        },
        pluginDirectoryRemover: @escaping (String) throws -> Void = {
            try FileManager.default.removeItem(atPath: $0)
        }
    ) {
        self.homeDirectory = homeDirectory
        self.pathEnvironment = pathEnvironment
        self.configurationWriter = configurationWriter
        self.pluginDirectoryRemover = pluginDirectoryRemover
    }

    init(
        homeDirectory: String = NSHomeDirectory(),
        pathEnvironment: String,
        configurationWriter: @escaping ([String: Any], String) throws -> Void = {
            try HookConfigWriter.write($0, to: $1)
        },
        pluginDirectoryRemover: @escaping (String) throws -> Void = {
            try FileManager.default.removeItem(atPath: $0)
        }
    ) {
        self.init(
            homeDirectory: homeDirectory,
            pathEnvironment: { pathEnvironment },
            configurationWriter: configurationWriter,
            pluginDirectoryRemover: pluginDirectoryRemover
        )
    }

    private var agentHome: String { homeDirectory + "/.xal" }
    private var pluginDirectory: String { agentHome + "/plugins/" + Self.pluginDirectoryName }
    private var pluginPath: String { pluginDirectory + "/" + Self.pluginFileName }
    private var configurationPath: String { agentHome + "/config.json" }

    private struct PluginState {
        let directoryExisted: Bool
        let data: Data?
        let permissions: Int?
    }

    private struct ConfigurationChange {
        let original: [String: Any]
        let updated: [String: Any]
    }

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
        let configuration = try configurationRegisteringPlugin()
        let previousPluginState = try capturePluginState()

        do {
            try installPlugin(sourceData)
            if let configuration {
                try configurationWriter(configuration, configurationPath)
            }
        } catch {
            try restorePluginState(previousPluginState)
            throw error
        }
    }

    func uninstall() throws {
        let configurationChange = try configurationUnregisteringPlugin()
        if let configurationChange {
            do {
                try configurationWriter(configurationChange.updated, configurationPath)
            } catch {
                try configurationWriter(configurationChange.original, configurationPath)
                throw error
            }
        }

        do {
            if FileManager.default.fileExists(atPath: pluginDirectory) {
                try pluginDirectoryRemover(pluginDirectory)
            }
        } catch {
            if let configurationChange {
                try configurationWriter(configurationChange.original, configurationPath)
            }
            throw error
        }
    }

    private func configurationRegisteringPlugin() throws -> [String: Any]? {
        var configuration = try readConfiguration()
        let plugins: [String]
        if let configuredPlugins = configuration["plugins"] {
            guard let configuredPlugins = configuredPlugins as? [String] else {
                throw XalProviderError.invalidPluginsConfiguration(configurationPath)
            }
            plugins = configuredPlugins
        } else {
            plugins = []
        }
        guard !plugins.contains(pluginDirectory) else { return nil }
        configuration["plugins"] = plugins + [pluginDirectory]
        return configuration
    }

    private func installPlugin(_ sourceData: Data) throws {
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
    }

    private func capturePluginState() throws -> PluginState {
        let directoryExisted = FileManager.default.fileExists(atPath: pluginDirectory)
        guard FileManager.default.fileExists(atPath: pluginPath) else {
            return PluginState(directoryExisted: directoryExisted, data: nil, permissions: nil)
        }
        let data = try Data(contentsOf: URL(fileURLWithPath: pluginPath))
        let attributes = try FileManager.default.attributesOfItem(atPath: pluginPath)
        let permissions = (attributes[.posixPermissions] as? NSNumber)?.intValue
        return PluginState(directoryExisted: directoryExisted, data: data, permissions: permissions)
    }

    private func restorePluginState(_ state: PluginState) throws {
        guard state.directoryExisted else {
            if FileManager.default.fileExists(atPath: pluginDirectory) {
                try FileManager.default.removeItem(atPath: pluginDirectory)
            }
            HookConfigWriteLedger.shared.reset(path: pluginPath)
            return
        }
        guard let data = state.data else {
            if FileManager.default.fileExists(atPath: pluginPath) {
                try FileManager.default.removeItem(atPath: pluginPath)
            }
            HookConfigWriteLedger.shared.reset(path: pluginPath)
            return
        }
        try data.write(to: URL(fileURLWithPath: pluginPath), options: .atomic)
        if let permissions = state.permissions {
            try FileManager.default.setAttributes([.posixPermissions: permissions], ofItemAtPath: pluginPath)
        }
        HookConfigWriteLedger.shared.recordWrite(path: pluginPath, contents: data)
    }

    private func configurationUnregisteringPlugin() throws -> ConfigurationChange? {
        guard FileManager.default.fileExists(atPath: configurationPath) else { return nil }
        let original = try readConfiguration()
        guard let configuredPlugins = original["plugins"] else { return nil }
        guard var plugins = configuredPlugins as? [String] else {
            throw XalProviderError.invalidPluginsConfiguration(configurationPath)
        }
        guard plugins.contains(pluginDirectory) else { return nil }
        plugins.removeAll { $0 == pluginDirectory }
        var updated = original
        if plugins.isEmpty {
            updated.removeValue(forKey: "plugins")
        } else {
            updated["plugins"] = plugins
        }
        return ConfigurationChange(original: original, updated: updated)
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
    case invalidPluginsConfiguration(String)

    var errorDescription: String? {
        switch self {
        case .hookResourceNotFound:
            "Xal plugin file (muxy-xal-plugin.ts) not found at the staged hook path"
        case let .malformedConfiguration(path):
            "Xal configuration at \(path) is not a JSON object"
        case let .invalidPluginsConfiguration(path):
            "Xal configuration at \(path) must define plugins as an array of strings"
        }
    }
}
