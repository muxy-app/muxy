import Foundation

struct OpenCodeProvider: AIProviderIntegration, AIAgentLaunchProvider {
    let id = "opencode"
    let displayName = "OpenCode"
    let socketTypeKey = "opencode"
    let iconName = "opencode"
    let executableNames = ["opencode"]

    var agentLaunchConfiguration: AIAgentLaunchConfiguration {
        AIAgentLaunchConfiguration(
            executable: "opencode",
            headlessArguments: ["run", "--pure"],
            environment: ["OPENCODE_PERMISSION": #"{"*":"deny"}"#]
        )
    }

    private static let pluginsDir = NSHomeDirectory() + "/.opencode/plugins"
    private static let pluginFileName = "muxy-notify.js"
    private static var pluginPath: String { pluginsDir + "/" + pluginFileName }
    private static let pluginScriptName = "opencode-muxy-plugin.js"

    func isToolInstalled() -> Bool {
        agentCLIExecutablePath() != nil
    }

    func agentCLIExecutablePath() -> String? {
        ProviderExecutableLocator.executablePath(
            names: [agentLaunchConfiguration.executable],
            homeDirectory: NSHomeDirectory(),
            pathEnvironment: LoginShellPath.current,
            includeSystemWide: true,
            homeRelativeBins: [".opencode/bin", ".local/bin"]
        )
    }

    func isHookInstalled() -> Bool {
        FileManager.default.fileExists(atPath: Self.pluginPath)
    }

    func install(hookScriptPath: String) throws {
        guard let sourcePlugin = Self.findPluginSource(near: hookScriptPath) else { return }
        let sourceData = try Data(contentsOf: URL(fileURLWithPath: sourcePlugin))

        if FileManager.default.fileExists(atPath: Self.pluginPath),
           let existingData = try? Data(contentsOf: URL(fileURLWithPath: Self.pluginPath)),
           existingData == sourceData
        {
            return
        }

        try FileManager.default.createDirectory(atPath: Self.pluginsDir, withIntermediateDirectories: true)
        let dest = URL(fileURLWithPath: Self.pluginPath)
        if FileManager.default.fileExists(atPath: Self.pluginPath) {
            try FileManager.default.removeItem(at: dest)
        }
        try FileManager.default.copyItem(at: URL(fileURLWithPath: sourcePlugin), to: dest)
    }

    func uninstall() throws {
        guard FileManager.default.fileExists(atPath: Self.pluginPath) else { return }
        try FileManager.default.removeItem(atPath: Self.pluginPath)
    }

    private static func findPluginSource(near hookScriptPath: String) -> String? {
        if let bundled = MuxyNotificationHooks.scriptPath(named: "opencode-muxy-plugin", extension: "js") {
            return bundled
        }

        let hookDir = (hookScriptPath as NSString).deletingLastPathComponent
        let candidate = (hookDir as NSString).appendingPathComponent(pluginScriptName)
        guard FileManager.default.fileExists(atPath: candidate) else { return nil }
        return candidate
    }
}
