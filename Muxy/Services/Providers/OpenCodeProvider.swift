import Foundation

struct OpenCodeProvider: AIProviderIntegration {
    let id = "opencode"
    let displayName = "OpenCode"
    let socketTypeKey = "opencode"
    let iconName = "chevron.left.forwardslash.chevron.right"
    let executableNames = ["opencode"]

    private static let pluginsDir = NSHomeDirectory() + "/.opencode/plugins"
    private static let pluginFileName = "muxy-notify.js"
    private static var pluginPath: String { pluginsDir + "/" + pluginFileName }
    private static let pluginScriptName = "opencode-muxy-plugin.js"

    func isToolInstalled() -> Bool {
        let home = NSHomeDirectory()
        let paths = [
            "\(home)/.opencode/bin/opencode",
            "\(home)/.local/bin/opencode",
            "/usr/local/bin/opencode",
            "/opt/homebrew/bin/opencode",
        ]
        return paths.contains { FileManager.default.isExecutableFile(atPath: $0) }
    }

    func install(hookScriptPath: String) throws {
        guard let sourcePlugin = findPluginSource(near: hookScriptPath) else { return }
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

    private func findPluginSource(near hookScriptPath: String) -> String? {
        let hookDir = (hookScriptPath as NSString).deletingLastPathComponent
        let candidate = (hookDir as NSString).appendingPathComponent(Self.pluginScriptName)
        guard FileManager.default.fileExists(atPath: candidate) else { return nil }
        return candidate
    }
}
