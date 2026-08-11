import Foundation

enum WorktreeConfigError: LocalizedError {
    case unreadable(path: String)
    case invalid(path: String)

    var errorDescription: String? {
        switch self {
        case let .unreadable(path):
            "Could not read worktree hook config at \(path)."
        case let .invalid(path):
            "Invalid worktree hook config at \(path)."
        }
    }
}

struct WorktreeConfig: Codable {
    struct SetupCommand: Codable {
        let command: String
        let name: String?

        init(command: String, name: String? = nil) {
            self.command = command
            self.name = name
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
        setup = Self.decodeCommands(from: container, forKey: .setup)
        teardown = Self.decodeCommands(from: container, forKey: .teardown)
    }

    private static func decodeCommands(
        from container: KeyedDecodingContainer<CodingKeys>,
        forKey key: CodingKeys
    ) -> [SetupCommand] {
        guard var array = try? container.nestedUnkeyedContainer(forKey: key) else { return [] }
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
        let global = try load(from: globalConfigURL)?.setup ?? []
        let project = try load(fromProjectPath: sourceProjectPath)?.setup ?? []
        return global + project
    }

    static func teardownCommands(sourceProjectPath: String, globalConfigURL: URL) throws -> [SetupCommand] {
        let project = try load(fromProjectPath: sourceProjectPath)?.teardown ?? []
        let global = try load(from: globalConfigURL)?.teardown ?? []
        return project + global
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
