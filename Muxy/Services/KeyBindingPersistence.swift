import Foundation

protocol KeyBindingPersisting {
    func loadBindings() throws -> [KeyBinding]
    func saveBindings(_ bindings: [KeyBinding]) throws
}

final class FileKeyBindingPersistence: KeyBindingPersisting {
    private let fileURL: URL

    init(fileURL: URL = MuxyFileStorage.fileURL(filename: "keybindings.json")) {
        self.fileURL = fileURL
    }

    func loadBindings() throws -> [KeyBinding] {
        guard FileManager.default.fileExists(atPath: fileURL.path) else {
            return KeyBinding.defaults
        }
        let data = try Data(contentsOf: fileURL)
        let containers = try JSONDecoder().decode([SafeKeyBinding].self, from: data)
        let saved = containers.compactMap(\.binding)
        return Self.mergeWithDefaults(saved)
    }

    func saveBindings(_ bindings: [KeyBinding]) throws {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        let data = try encoder.encode(bindings)
        try data.write(to: fileURL, options: .atomic)
        try FileManager.default.setAttributes(
            [.posixPermissions: 0o600],
            ofItemAtPath: fileURL.path
        )
    }

    private static func mergeWithDefaults(_ saved: [KeyBinding]) -> [KeyBinding] {
        let savedByAction = Dictionary(uniqueKeysWithValues: saved.map { ($0.action, $0) })
        return KeyBinding.defaults.map { defaultBinding in
            savedByAction[defaultBinding.action] ?? defaultBinding
        }
    }

    private struct SafeKeyBinding: Decodable {
        let binding: KeyBinding?

        init(from decoder: Decoder) throws {
            binding = try? KeyBinding(from: decoder)
        }
    }
}
