import Foundation

enum MuxyAgentBin {
    static let wrapperName = "pi"
    static let extensionFileName = "muxy-pi-extension.ts"

    static func directory(appSupportDirectory: URL = MuxyFileStorage.appSupportDirectory()) -> URL {
        appSupportDirectory.appendingPathComponent("bin", isDirectory: true)
    }

    static func wrapperURL(appSupportDirectory: URL = MuxyFileStorage.appSupportDirectory()) -> URL {
        directory(appSupportDirectory: appSupportDirectory).appendingPathComponent(wrapperName)
    }

    static func extensionURL(appSupportDirectory: URL = MuxyFileStorage.appSupportDirectory()) -> URL {
        directory(appSupportDirectory: appSupportDirectory).appendingPathComponent(extensionFileName)
    }

    static func pathPrefixForTerminal(
        appSupportDirectory: URL = MuxyFileStorage.appSupportDirectory(),
        fileManager: FileManager = .default
    ) -> String? {
        let wrapperPath = wrapperURL(appSupportDirectory: appSupportDirectory).path
        guard fileManager.isExecutableFile(atPath: wrapperPath) else { return nil }
        return directory(appSupportDirectory: appSupportDirectory).path
    }
}
