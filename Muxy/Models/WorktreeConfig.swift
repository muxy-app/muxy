import Foundation

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

    private enum CodingKeys: String, CodingKey {
        case setup
    }

    init(setup: [SetupCommand]) {
        self.setup = setup
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        if let objectEntries = try? container.decode([SetupCommand].self, forKey: .setup) {
            setup = objectEntries
            return
        }
        if let stringEntries = try? container.decode([String].self, forKey: .setup) {
            setup = stringEntries.map { SetupCommand(command: $0) }
            return
        }
        setup = []
    }

    static func load(fromProjectPath projectPath: String) -> WorktreeConfig? {
        let url = URL(fileURLWithPath: projectPath)
            .appendingPathComponent(".muxy")
            .appendingPathComponent("worktree.json")
        guard FileManager.default.fileExists(atPath: url.path) else { return nil }
        guard let data = try? Data(contentsOf: url) else { return nil }
        return try? JSONDecoder().decode(WorktreeConfig.self, from: data)
    }
}
