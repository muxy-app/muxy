import Foundation

enum ProviderExecutableLocator {
    static func candidateDirectories(
        homeDirectory: String,
        pathEnvironment: String,
        includeSystemWide: Bool,
        homeRelativeBins: [String] = [".local/bin"]
    ) -> [String] {
        let pathDirectories = pathEnvironment
            .split(separator: ":", omittingEmptySubsequences: true)
            .map(String.init)

        var directories = homeRelativeBins.map { "\(homeDirectory)/\($0)" }
        if includeSystemWide {
            directories.append(contentsOf: [
                "/usr/local/bin",
                "/opt/homebrew/bin",
            ])
        }
        directories.append(contentsOf: pathDirectories)

        var seen = Set<String>()
        return directories.filter { seen.insert($0).inserted }
    }

    static func isInstalled(
        names: [String],
        homeDirectory: String,
        pathEnvironment: String,
        includeSystemWide: Bool,
        homeRelativeBins: [String] = [".local/bin"],
        isExecutable: (String) -> Bool = { FileManager.default.isExecutableFile(atPath: $0) }
    ) -> Bool {
        executablePath(
            names: names,
            homeDirectory: homeDirectory,
            pathEnvironment: pathEnvironment,
            includeSystemWide: includeSystemWide,
            homeRelativeBins: homeRelativeBins,
            isExecutable: isExecutable
        ) != nil
    }

    static func executablePath(
        names: [String],
        homeDirectory: String,
        pathEnvironment: String,
        includeSystemWide: Bool,
        homeRelativeBins: [String] = [".local/bin"],
        isExecutable: (String) -> Bool = { FileManager.default.isExecutableFile(atPath: $0) }
    ) -> String? {
        let directories = candidateDirectories(
            homeDirectory: homeDirectory,
            pathEnvironment: pathEnvironment,
            includeSystemWide: includeSystemWide,
            homeRelativeBins: homeRelativeBins
        )
        for name in names {
            for directory in directories {
                let path = URL(fileURLWithPath: directory).appendingPathComponent(name).path
                if isExecutable(path) {
                    return path
                }
            }
        }
        return nil
    }
}
