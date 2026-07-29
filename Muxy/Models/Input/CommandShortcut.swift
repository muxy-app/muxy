import Foundation

struct CommandShortcut: Codable, Identifiable, Equatable {
    var id: UUID
    var name: String
    var command: String
    var combo: KeyCombo

    init(
        id: UUID = UUID(),
        name: String = "",
        command: String = "",
        combo: KeyCombo = KeyCombo(key: "t", command: true, option: true)
    ) {
        self.id = id
        self.name = name
        self.command = command
        self.combo = combo
    }

    var displayName: String {
        let trimmedName = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedName.isEmpty else { return "Command" }
        return trimmedName
    }

    var trimmedCommand: String {
        command.trimmingCharacters(in: .whitespacesAndNewlines)
    }
}

@MainActor
extension CommandShortcut {
    var localizedDisplayName: String {
        localizedDisplayName(using: .shared)
    }

    func localizedDisplayName(using localization: LocalizationService) -> String {
        name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            ? localization.searchString(key: "Command")
            : displayName
    }

    func matchesSearch(
        query: String,
        localization: LocalizationService = .shared
    ) -> Bool {
        LocalizedSearch.matches(
            query: query,
            localizedKeys: name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? ["Command"] : [],
            verbatimValues: [name, command],
            localization: localization
        )
    }
}
