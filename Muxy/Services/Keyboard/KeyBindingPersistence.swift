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
                filePermissions: FilePermissions.privateFile
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
        var savedByAction: [ShortcutAction: KeyBinding] = [:]
        var claimedCombos = Set(saved.map(\.combo).filter(\.isAssigned))
        for binding in saved {
            savedByAction[binding.action] = binding
        }
        return KeyBinding.defaults.map { defaultBinding in
            if let savedBinding = savedByAction[defaultBinding.action] {
                return savedBinding
            }
            guard defaultBinding.combo.isAssigned else { return defaultBinding }
            guard !claimedCombos.contains(defaultBinding.combo) else {
                return KeyBinding(action: defaultBinding.action, combo: KeyCombo(key: "", modifiers: 0))
            }
            claimedCombos.insert(defaultBinding.combo)
            return defaultBinding
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
