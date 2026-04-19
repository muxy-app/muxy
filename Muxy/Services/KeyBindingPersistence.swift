import Foundation

protocol KeyBindingPersisting {
    func loadBindings() throws -> [KeyBinding]
    func saveBindings(_ bindings: [KeyBinding]) throws
}

final class FileKeyBindingPersistence: KeyBindingPersisting {
    private let reader: CodableFileStore<[SafeKeyBinding]>
    private let writer: CodableFileStore<[KeyBinding]>

    init(fileURL: URL = MuxyFileStorage.fileURL(filename: "keybindings.json")) {
        reader = CodableFileStore(fileURL: fileURL)
        writer = CodableFileStore(
            fileURL: fileURL,
            options: CodableFileStoreOptions(
                prettyPrinted: true,
                sortedKeys: true,
                filePermissions: 0o600
            )
        )
    }

    func loadBindings() throws -> [KeyBinding] {
        guard let containers = try reader.load() else { return KeyBinding.defaults }
        return Self.mergeWithDefaults(containers.compactMap(\.binding))
    }

    func saveBindings(_ bindings: [KeyBinding]) throws {
        try writer.save(bindings)
    }

    private static func mergeWithDefaults(_ saved: [KeyBinding]) -> [KeyBinding] {
        let savedByAction = Dictionary(uniqueKeysWithValues: saved.map { ($0.action, $0) })
        return KeyBinding.defaults.map { defaultBinding in
            savedByAction[defaultBinding.action] ?? defaultBinding
        }
    }

    private struct SafeKeyBinding: Codable {
        let binding: KeyBinding?

        init(from decoder: Decoder) throws {
            binding = try? KeyBinding(from: decoder)
        }

        func encode(to encoder: Encoder) throws {
            try binding?.encode(to: encoder)
        }
    }
}
