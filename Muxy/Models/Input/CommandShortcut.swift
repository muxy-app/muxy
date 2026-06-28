import Foundation

struct CommandShortcut: Codable, Identifiable, Equatable {
    var id: UUID
    var name: String
    var command: String
    var combo: KeyCombo

    init(
        id: UUID = UUID(),
        name: String = "New Command",
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
