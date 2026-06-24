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
        let directories = candidateDirectories(
            homeDirectory: homeDirectory,
            pathEnvironment: pathEnvironment,
            includeSystemWide: includeSystemWide,
            homeRelativeBins: homeRelativeBins
        )
        return names.contains { name in
            directories.contains { directory in
                let path = URL(fileURLWithPath: directory).appendingPathComponent(name).path
                return isExecutable(path)
            }
        }
    }
}
