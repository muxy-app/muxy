import Foundation

enum WorktreeConfigError: LocalizedError {
    case unreadable(path: String)
    case invalid(path: String)
    case projectHooksChanged

    var errorDescription: String? {
        switch self {
        case let .unreadable(path):
            "Could not read worktree hook config at \(path)."
        case let .invalid(path):
            "Invalid worktree hook config at \(path)."
        case .projectHooksChanged:
            "Project worktree hooks changed after approval. Review them and try again."
        }
    }
}

struct WorktreeConfig: Codable {
    struct SetupCommand: Codable, Hashable {
        let command: String
        let name: String?

        init(command: String, name: String? = nil) {
            self.command = command
            self.name = name
        }
    }

    enum CommandSource: Hashable {
        case global
        case project
    }

    struct ResolvedCommand: Hashable {
        let command: SetupCommand
        let source: CommandSource
    }

    struct ProjectHookApproval: Hashable {
        let commands: [SetupCommand]

        init(resolvedCommands: [ResolvedCommand]) {
            commands = WorktreeConfig.normalizedCommands(resolvedCommands)
                .filter { $0.source == .project }
                .map(\.command)
        }
    }

    let setup: [SetupCommand]
    let teardown: [SetupCommand]

    private enum CodingKeys: String, CodingKey {
        case setup
        case teardown
    }

    init(setup: [SetupCommand], teardown: [SetupCommand] = []) {
        self.setup = setup
        self.teardown = teardown
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        setup = try Self.decodeCommands(from: container, forKey: .setup)
        teardown = try Self.decodeCommands(from: container, forKey: .teardown)
    }

    private static func decodeCommands(
        from container: KeyedDecodingContainer<CodingKeys>,
        forKey key: CodingKeys
    ) throws -> [SetupCommand] {
        guard container.contains(key) else { return [] }
        var array = try container.nestedUnkeyedContainer(forKey: key)
        var commands: [SetupCommand] = []
        while !array.isAtEnd {
            if let command = try? array.decode(SetupCommand.self) {
                commands.append(command)
                continue
            }
            if let string = try? array.decode(String.self) {
                commands.append(SetupCommand(command: string))
                continue
            }
            _ = try? array.decode(EmptyEntry.self)
        }
        return commands
    }

    private struct EmptyEntry: Decodable {}

    static func load(fromProjectPath projectPath: String) throws -> WorktreeConfig? {
        let url = URL(fileURLWithPath: projectPath)
            .appendingPathComponent(".muxy")
            .appendingPathComponent("worktree.json")
        return try load(from: url)
    }

    static func globalConfigURL(
        homeDirectory: URL = FileManager.default.homeDirectoryForCurrentUser,
        environment: [String: String] = ProcessInfo.processInfo.environment
    ) -> URL {
        let configDirectory = environment["XDG_CONFIG_HOME"].flatMap { $0.isEmpty ? nil : $0 }
            .map { URL(fileURLWithPath: $0, isDirectory: true) }
            ?? homeDirectory.appendingPathComponent(".config", isDirectory: true)
        return configDirectory
            .appendingPathComponent("muxy", isDirectory: true)
            .appendingPathComponent("worktree.json")
    }

    static func setupCommands(sourceProjectPath: String, globalConfigURL: URL) throws -> [SetupCommand] {
        try resolvedSetupCommands(
            sourceProjectPath: sourceProjectPath,
            globalConfigURL: globalConfigURL
        ).map(\.command)
    }

    static func resolvedSetupCommands(
        sourceProjectPath: String,
        globalConfigURL: URL,
        includeProjectCommands: Bool = true
    ) throws -> [ResolvedCommand] {
        let global = try load(from: globalConfigURL)?.setup ?? []
        guard includeProjectCommands else {
            return global.map { ResolvedCommand(command: $0, source: .global) }
        }
        let project = try load(fromProjectPath: sourceProjectPath)?.setup ?? []
        return global.map { ResolvedCommand(command: $0, source: .global) }
            + project.map { ResolvedCommand(command: $0, source: .project) }
    }

    static func teardownCommands(sourceProjectPath: String, globalConfigURL: URL) throws -> [SetupCommand] {
        try resolvedTeardownCommands(
            sourceProjectPath: sourceProjectPath,
            globalConfigURL: globalConfigURL
        ).map(\.command)
    }

    static func resolvedTeardownCommands(
        sourceProjectPath: String,
        globalConfigURL: URL,
        includeProjectCommands: Bool = true
    ) throws -> [ResolvedCommand] {
        let global = try load(from: globalConfigURL)?.teardown ?? []
        guard includeProjectCommands else {
            return global.map { ResolvedCommand(command: $0, source: .global) }
        }
        let project = try load(fromProjectPath: sourceProjectPath)?.teardown ?? []
        return project.map { ResolvedCommand(command: $0, source: .project) }
            + global.map { ResolvedCommand(command: $0, source: .global) }
    }

    static func commandsForExecution(
        _ commands: [ResolvedCommand],
        projectHookApproval: ProjectHookApproval?
    ) throws -> [String] {
        let normalized = normalizedCommands(commands)
        guard let projectHookApproval else {
            return normalized.filter { $0.source == .global }.map(\.command.command)
        }
        let projectCommands = normalized.filter { $0.source == .project }.map(\.command)
        guard projectCommands == projectHookApproval.commands else {
            throw WorktreeConfigError.projectHooksChanged
        }
        return normalized.map(\.command.command)
    }

    static func normalizedCommands(_ commands: [ResolvedCommand]) -> [ResolvedCommand] {
        commands.compactMap { resolved in
            let command = resolved.command.command.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !command.isEmpty else { return nil }
            return ResolvedCommand(
                command: SetupCommand(command: command, name: resolved.command.name),
                source: resolved.source
            )
        }
    }

    private static func load(from url: URL) throws -> WorktreeConfig? {
        guard FileManager.default.fileExists(atPath: url.path) else { return nil }
        let data: Data
        do {
            data = try Data(contentsOf: url)
        } catch {
            throw WorktreeConfigError.unreadable(path: url.path)
        }
        do {
            return try JSONDecoder().decode(WorktreeConfig.self, from: data)
        } catch {
            throw WorktreeConfigError.invalid(path: url.path)
        }
    }
}
