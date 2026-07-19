import Foundation

public enum AgentHookPaths {
    static var applicationSupportDirectory: URL? {
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first?
            .appendingPathComponent("Muxy", isDirectory: true)
    }

    static var defaultSocketPath: String? {
        applicationSupportDirectory?.appendingPathComponent(socketFileName).path
    }

    public static var defaultLogFileURL: URL? {
        applicationSupportDirectory?.appendingPathComponent("hooks.log")
    }

    private static var socketFileName: String {
        #if DEBUG
        "muxy-dev.sock"
        #else
        "muxy.sock"
        #endif
    }
}
