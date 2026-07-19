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

    func install(hookScriptPath: String) throws {
        let sourceData = try Data(contentsOf: URL(fileURLWithPath: hookScriptPath))

        if FileManager.default.fileExists(atPath: pluginPath),
           let existingData = try? Data(contentsOf: URL(fileURLWithPath: pluginPath)),
           existingData == sourceData
        {
            return
        }

        try FileManager.default.createDirectory(
            atPath: pluginsDirectory,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: FilePermissions.privateDirectory]
        )
        try sourceData.write(to: URL(fileURLWithPath: pluginPath), options: .atomic)
        try FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.privateFile],
            ofItemAtPath: pluginPath
        )
    }

    func uninstall() throws {
        guard FileManager.default.fileExists(atPath: pluginPath) else { return }
        try FileManager.default.removeItem(atPath: pluginPath)
    }
}
