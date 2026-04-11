import Foundation

struct WorktreeConfig: Codable {
    struct SetupCommand: Codable {
        let command: String
        let name: String?
    }

    let setup: [SetupCommand]

    static func load(fromProjectPath projectPath: String) -> WorktreeConfig? {
        let url = URL(fileURLWithPath: projectPath)
            .appendingPathComponent(".muxy")
            .appendingPathComponent("worktree.json")
        guard FileManager.default.fileExists(atPath: url.path) else { return nil }
        guard let data = try? Data(contentsOf: url) else { return nil }
        return try? JSONDecoder().decode(WorktreeConfig.self, from: data)
    }
}
