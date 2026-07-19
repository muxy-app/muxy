import Foundation

struct OpenCodeProvider: AIProviderIntegration, AIAgentLaunchProvider {
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

    private var pluginsDirectory: String { homeDirectory + "/.opencode/plugins" }
    private var pluginPath: String { pluginsDirectory + "/" + Self.pluginFileName }

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

    func isHookInstalled() -> Bool {
        FileManager.default.fileExists(atPath: pluginPath)
    }

    var configPaths: [String] { [pluginPath] }

    func verify(hookScriptPath: String) -> HookVerification {
        guard FileManager.default.fileExists(atPath: pluginPath) else { return .needsRepair }
        guard FileManager.default.contentsEqual(atPath: hookScriptPath, andPath: pluginPath) else {
            return .needsRepair
        }
        guard installedPluginHasPrivatePermissions() else { return .needsRepair }
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
    }

    func uninstall() throws {
        guard FileManager.default.fileExists(atPath: pluginPath) else { return }
        try FileManager.default.removeItem(atPath: pluginPath)
    }

    private func installedPluginHasPrivatePermissions() -> Bool {
        guard let attributes = try? FileManager.default.attributesOfItem(atPath: pluginPath),
              let permissions = attributes[.posixPermissions] as? NSNumber
        else { return false }
        return permissions.intValue == FilePermissions.privateFile
    }
}
