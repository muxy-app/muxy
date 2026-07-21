import Foundation
import Observation
import os

private let notchTerminalShortcutLogger = Logger(
    subsystem: "app.muxy",
    category: "NotchTerminalShortcutStore"
)

enum NotchTerminalShortcutError: LocalizedError, Equatable {
    case invalidShortcut
    case carbonEventHandlerInstallationFailed(OSStatus)
    case carbonHotKeyRegistrationFailed(OSStatus)

    var errorDescription: String? {
        switch self {
        case .invalidShortcut:
            "Choose a supported key with Command, Control, or Option."
        case let .carbonEventHandlerInstallationFailed(status):
            "The global shortcut event handler could not be installed (\(status))."
        case let .carbonHotKeyRegistrationFailed(status):
            "The global shortcut could not be registered (\(status))."
        }
    }
}

protocol NotchTerminalShortcutPersisting {
    func loadShortcut() throws -> NotchTerminalShortcut
    func saveShortcut(_ shortcut: NotchTerminalShortcut) throws
}

final class FileNotchTerminalShortcutPersistence: NotchTerminalShortcutPersisting {
    private let store: CodableFileStore<NotchTerminalShortcut>

    init(fileURL: URL = MuxyFileStorage.fileURL(filename: "notch-terminal-shortcut.json")) {
        store = CodableFileStore(
            fileURL: fileURL,
            options: CodableFileStoreOptions(
                prettyPrinted: true,
                sortedKeys: true,
                filePermissions: FilePermissions.privateFile
            )
        )
    }

    func loadShortcut() throws -> NotchTerminalShortcut {
        try store.load() ?? .default
    }

    func saveShortcut(_ shortcut: NotchTerminalShortcut) throws {
        try store.save(shortcut)
    }
}

@MainActor
@Observable
final class NotchTerminalShortcutStore {
    typealias PersistenceCommit = @MainActor @Sendable () throws -> Void
    typealias ChangeHandler = @MainActor (NotchTerminalShortcut, PersistenceCommit) throws -> Void

    static let shared = NotchTerminalShortcutStore()

    private(set) var shortcut = NotchTerminalShortcut.default
    private let persistence: any NotchTerminalShortcutPersisting
    @ObservationIgnored private let settingsSynchronizer: @MainActor () -> Void
    @ObservationIgnored private var changeHandler: ChangeHandler?

    init(
        persistence: any NotchTerminalShortcutPersisting = FileNotchTerminalShortcutPersistence(),
        settingsSynchronizer: @escaping @MainActor () -> Void = {
            SettingsJSONStore.syncUserSettingsFileWithCurrentSettings()
        }
    ) {
        self.persistence = persistence
        self.settingsSynchronizer = settingsSynchronizer
        load()
    }

    func updateShortcut(_ newShortcut: NotchTerminalShortcut) throws {
        guard newShortcut.isValid else { throw NotchTerminalShortcutError.invalidShortcut }
        guard newShortcut != shortcut else { return }

        let persistenceCommit: PersistenceCommit = {
            try self.persistence.saveShortcut(newShortcut)
        }
        if let changeHandler {
            try changeHandler(newShortcut, persistenceCommit)
        } else {
            try persistenceCommit()
        }
        shortcut = newShortcut
        settingsSynchronizer()
    }

    func resetToDefault() throws {
        try updateShortcut(.default)
    }

    func setChangeHandler(_ handler: ChangeHandler?) {
        changeHandler = handler
    }

    private func load() {
        do {
            let storedShortcut = try persistence.loadShortcut()
            guard storedShortcut.isValid else {
                shortcut = .default
                return
            }
            shortcut = storedShortcut
        } catch {
            notchTerminalShortcutLogger.error(
                "Failed to load the notch terminal shortcut: \(error.localizedDescription)"
            )
            shortcut = .default
        }
    }
}
