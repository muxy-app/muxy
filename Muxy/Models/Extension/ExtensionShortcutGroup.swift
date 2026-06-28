import Foundation

struct ExtensionShortcutEntry: Identifiable, Equatable {
    let extensionID: String
    let commandID: String
    let commandTitle: String
    let combo: KeyCombo
    let defaultCombo: KeyCombo?

    var id: String { "\(extensionID):\(commandID)" }
}

struct ExtensionShortcutGroup: Identifiable, Equatable {
    let extensionID: String
    let extensionName: String
    let entries: [ExtensionShortcutEntry]

    var id: String { extensionID }

    @MainActor
    static func build(
        shortcuts: [ExtensionShortcut],
        statuses: [ExtensionStore.ExtensionStatus]
    ) -> [ExtensionShortcutGroup] {
        let enabled = statuses.filter(\.isEnabled)
        return enabled.compactMap { status in
            let manifest = status.muxyExtension.manifest
            let entries = manifest.commands.compactMap { command -> ExtensionShortcutEntry? in
                guard command.defaultShortcut != nil,
                      let shortcut = shortcuts.first(where: {
                          $0.extensionID == status.id && $0.commandID == command.id
                      })
                else { return nil }
                return ExtensionShortcutEntry(
                    extensionID: status.id,
                    commandID: command.id,
                    commandTitle: command.title,
                    combo: shortcut.combo,
                    defaultCombo: command.defaultCombo
                )
            }
            guard !entries.isEmpty else { return nil }
            return ExtensionShortcutGroup(
                extensionID: status.id,
                extensionName: status.muxyExtension.displayName,
                entries: entries
            )
        }
    }
}
