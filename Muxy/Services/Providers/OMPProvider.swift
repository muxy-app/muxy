import Foundation

struct OMPProvider: AIProviderIntegration, AIAgentLaunchProvider, AIProviderDiscoveryIntegration, Sendable {
    let id = "omp"
    let displayName = "Oh My Pi"
    let socketTypeKey = "omp"
    let iconName = "omp"
    let executableNames = ["omp"]
    let hookScriptName = "muxy-omp-extension"
    let hookScriptExtension = "ts"

    var agentLaunchConfiguration: AIAgentLaunchConfiguration {
        AIAgentLaunchConfiguration(
            executable: "omp",
            headlessArguments: [
                "-p",
                "--no-session",
                "--no-tools",
                "--no-title",
                "--mode",
                "text",
            ],
            modelArgument: "--model",
            environment: ["PI_NO_PTY": "1"]
        )
    }

    private static let destinationFileName = "muxy-notify.ts"
    private let homeDirectory: String
    private let pathEnvironment: @Sendable () -> String
    private let environment: @Sendable () -> [String: String]

    init(
        homeDirectory: String = NSHomeDirectory(),
        pathEnvironment: @escaping @Sendable () -> String = { LoginShellPath.current },
        environment: @escaping @Sendable () -> [String: String] = { ProcessInfo.processInfo.environment }
    ) {
        self.homeDirectory = homeDirectory
        self.pathEnvironment = pathEnvironment
        self.environment = environment
    }

    init(
        homeDirectory: String = NSHomeDirectory(),
        pathEnvironment: String,
        environment: @escaping @Sendable () -> [String: String] = { ProcessInfo.processInfo.environment }
    ) {
        self.init(homeDirectory: homeDirectory, pathEnvironment: { pathEnvironment }, environment: environment)
    }

    private var configRoot: URL {
        guard let configured = environment()["PI_CONFIG_DIR"]?.trimmingCharacters(in: .whitespacesAndNewlines),
              !configured.isEmpty
        else {
            return URL(fileURLWithPath: homeDirectory).appendingPathComponent(".omp")
        }
        let expanded = (configured as NSString).expandingTildeInPath
        return expanded.hasPrefix("/")
            ? URL(fileURLWithPath: expanded).standardizedFileURL
            : URL(fileURLWithPath: homeDirectory).appendingPathComponent(expanded).standardizedFileURL
    }

    private var profilesRoot: URL {
        configRoot.appendingPathComponent("profiles")
    }

    private var primaryAgentDirectory: URL {
        let values = environment()
        if let requested = values["OMP_PROFILE"] ?? values["PI_PROFILE"] {
            let profile = requested.trimmingCharacters(in: .whitespacesAndNewlines)
            if profile != "default",
               profile.range(of: #"^[a-z0-9][a-z0-9._-]{0,63}$"#, options: .regularExpression) != nil
            {
                return profilesRoot.appendingPathComponent("\(profile)/agent").standardizedFileURL
            }
        }
        if let custom = values["PI_CODING_AGENT_DIR"]?.trimmingCharacters(in: .whitespacesAndNewlines),
           !custom.isEmpty
        {
            let expanded = (custom as NSString).expandingTildeInPath
            return expanded.hasPrefix("/")
                ? URL(fileURLWithPath: expanded).standardizedFileURL
                : URL(fileURLWithPath: homeDirectory).appendingPathComponent(expanded).standardizedFileURL
        }
        return configRoot.appendingPathComponent("agent")
    }

    private var agentDirectories: [URL] {
        var directories = [primaryAgentDirectory]
        if let profiles = try? FileManager.default.contentsOfDirectory(
            at: profilesRoot,
            includingPropertiesForKeys: [.isDirectoryKey],
            options: [.skipsHiddenFiles]
        ) {
            for profile in profiles.sorted(by: { $0.lastPathComponent < $1.lastPathComponent }) {
                let name = profile.lastPathComponent
                guard name.range(of: #"^[a-z0-9][a-z0-9._-]{0,63}$"#, options: .regularExpression) != nil,
                      (try? profile.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true
                else { continue }
                directories.append(profile.appendingPathComponent("agent"))
            }
        }
        var seen = Set<String>()
        return directories.filter { seen.insert($0.standardizedFileURL.path).inserted }
    }

    private var destinationPaths: [String] {
        agentDirectories.map {
            $0.appendingPathComponent("extensions/\(Self.destinationFileName)").standardizedFileURL.path
        }
    }

    var discoveryArguments: [String] { ["--version"] }
    var discoveryWorkingDirectory: String { homeDirectory }

    func discoveryDetails(from output: String) -> ProviderDiscoveryDetails {
        let trimmed = output.trimmingCharacters(in: .whitespacesAndNewlines)
        if let match = try? Regex(#"(?:^|[\s/vV])(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)\b"#).firstMatch(in: trimmed),
           let capture = match.output[1].substring
        {
            return ProviderDiscoveryDetails(version: String(capture), state: .ready)
        }
        return ProviderDiscoveryDetails(version: trimmed.isEmpty ? nil : trimmed, state: .ready)
    }

    func isToolInstalled() -> Bool {
        agentCLIExecutablePath() != nil
    }

    func agentCLIExecutablePath() -> String? {
        ProviderExecutableLocator.executablePath(
            names: [agentLaunchConfiguration.executable],
            homeDirectory: homeDirectory,
            pathEnvironment: pathEnvironment(),
            includeSystemWide: homeDirectory == NSHomeDirectory(),
            homeRelativeBins: [".bun/bin", ".local/bin", ".npm-global/bin", ".cargo/bin"]
        )
    }

    func isHookInstalled() -> Bool {
        destinationPaths.allSatisfy { FileManager.default.fileExists(atPath: $0) }
    }

    func hasManagedState() -> Bool {
        destinationPaths.contains { path in
            FileManager.default.fileExists(atPath: path) ||
                (try? FileManager.default.destinationOfSymbolicLink(atPath: path)) != nil
        }
    }

    var configPaths: [String] {
        destinationPaths
    }

    func verify(hookScriptPath: String) -> HookVerification {
        let satisfied = destinationPaths.allSatisfy { destinationPath in
            FileManager.default.fileExists(atPath: destinationPath) &&
                FileManager.default.contentsEqual(atPath: hookScriptPath, andPath: destinationPath) &&
                installedExtensionHasPrivatePermissions(at: destinationPath)
        }
        return satisfied ? .satisfied : .needsRepair
    }

    func install(hookScriptPath: String) throws {
        let sourceURL = URL(fileURLWithPath: hookScriptPath)
        guard FileManager.default.fileExists(atPath: sourceURL.path) else {
            throw OMPProviderError.hookResourceNotFound
        }
        let sourceData = try Data(contentsOf: sourceURL)

        for destinationPath in destinationPaths {
            if (try? FileManager.default.destinationOfSymbolicLink(atPath: destinationPath)) != nil {
                try FileManager.default.removeItem(atPath: destinationPath)
            }
            try ensurePrivateExtensionsDirectory(for: destinationPath)
            if let existingData = try? Data(contentsOf: URL(fileURLWithPath: destinationPath)),
               existingData == sourceData
            {
                try setPrivatePermissions(at: destinationPath)
                HookConfigWriteLedger.shared.recordWrite(path: destinationPath, contents: sourceData)
                continue
            }
            try sourceData.write(to: URL(fileURLWithPath: destinationPath), options: .atomic)
            try setPrivatePermissions(at: destinationPath)
            HookConfigWriteLedger.shared.recordWrite(path: destinationPath, contents: sourceData)
        }
    }

    func uninstall() throws {
        for destinationPath in destinationPaths {
            do {
                try FileManager.default.removeItem(atPath: destinationPath)
            } catch let error as CocoaError where error.code == .fileNoSuchFile {}
        }
    }

    private func ensurePrivateExtensionsDirectory(for destinationPath: String) throws {
        let extensionsDir = (destinationPath as NSString).deletingLastPathComponent
        try FileManager.default.createDirectory(
            atPath: extensionsDir,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: FilePermissions.privateDirectory]
        )
        try FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.privateDirectory],
            ofItemAtPath: extensionsDir
        )
    }

    private func setPrivatePermissions(at destinationPath: String) throws {
        try FileManager.default.setAttributes(
            [.posixPermissions: FilePermissions.privateFile],
            ofItemAtPath: destinationPath
        )
    }

    private func installedExtensionHasPrivatePermissions(at destinationPath: String) -> Bool {
        guard (try? FileManager.default.destinationOfSymbolicLink(atPath: destinationPath)) == nil,
              let attributes = try? FileManager.default.attributesOfItem(atPath: destinationPath),
              attributes[.type] as? FileAttributeType == .typeRegular,
              let permissions = attributes[.posixPermissions] as? NSNumber
        else { return false }
        return permissions.intValue == FilePermissions.privateFile
    }
}

enum OMPProviderError: LocalizedError, Equatable {
    case hookResourceNotFound

    var errorDescription: String? {
        switch self {
        case .hookResourceNotFound:
            "OMP extension file (muxy-omp-extension.ts) not found at the staged hook path"
        }
    }
}
