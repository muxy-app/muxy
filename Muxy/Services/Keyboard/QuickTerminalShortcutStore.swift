import Foundation
import Observation
import os

private let quickTerminalShortcutLogger = Logger(
    subsystem: "app.muxy",
    category: "QuickTerminalShortcutStore"
)

enum QuickTerminalShortcutError: LocalizedError, Equatable {
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

protocol QuickTerminalShortcutPersisting {
    func loadShortcut() throws -> QuickTerminalShortcut
    func saveShortcut(_ shortcut: QuickTerminalShortcut) throws
}

final class FileQuickTerminalShortcutPersistence: QuickTerminalShortcutPersisting {
    private let store: CodableFileStore<QuickTerminalShortcut>

    init(fileURL: URL = MuxyFileStorage.fileURL(filename: "quick-terminal-shortcut.json")) {
        store = CodableFileStore(
            fileURL: fileURL,
            options: CodableFileStoreOptions(
                prettyPrinted: true,
                sortedKeys: true,
                filePermissions: FilePermissions.privateFile
            )
        )
    }

    func loadShortcut() throws -> QuickTerminalShortcut {
        try store.load() ?? .default
    }

    func saveShortcut(_ shortcut: QuickTerminalShortcut) throws {
        try store.save(shortcut)
    }
}

@MainActor
@Observable
final class QuickTerminalShortcutStore {
    typealias PersistenceCommit = @MainActor @Sendable () throws -> Void
    typealias ChangeHandler = @MainActor (QuickTerminalShortcut, PersistenceCommit) throws -> Void
    typealias Canonicalizer = @MainActor (QuickTerminalShortcut) -> QuickTerminalShortcut?

    static let shared = QuickTerminalShortcutStore()

    private(set) var shortcut = QuickTerminalShortcut.default
    private let persistence: any QuickTerminalShortcutPersisting
    @ObservationIgnored private let settingsSynchronizer: @MainActor () -> Void
    @ObservationIgnored private let canonicalizer: Canonicalizer
    @ObservationIgnored private var changeHandler: ChangeHandler?

    init(
        persistence: any QuickTerminalShortcutPersisting = FileQuickTerminalShortcutPersistence(),
        settingsSynchronizer: @escaping @MainActor () -> Void = {
            SettingsJSONStore.syncUserSettingsFileWithCurrentSettings()
        },
        canonicalizer: @escaping Canonicalizer = {
            $0.canonicalizedForCurrentKeyboardLayout()
        }
    ) {
        self.persistence = persistence
        self.settingsSynchronizer = settingsSynchronizer
        self.canonicalizer = canonicalizer
        load()
    }

    func updateShortcut(_ newShortcut: QuickTerminalShortcut) throws {
        guard let newShortcut = canonicalized(newShortcut) else {
            throw QuickTerminalShortcutError.invalidShortcut
        }
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

    func canonicalized(_ shortcut: QuickTerminalShortcut) -> QuickTerminalShortcut? {
        canonicalizer(shortcut)
    }

    private func load() {
        do {
            let persistedShortcut = try persistence.loadShortcut()
            guard let storedShortcut = canonicalized(persistedShortcut) else {
                shortcut = .default
                return
            }
            if storedShortcut != persistedShortcut {
                do {
                    try persistence.saveShortcut(storedShortcut)
                } catch {
                    quickTerminalShortcutLogger.error(
                        "Failed to normalize the stored shortcut: \(error.localizedDescription)"
                    )
                }
            }
            shortcut = storedShortcut
        } catch {
            quickTerminalShortcutLogger.error(
                "Failed to load the quick terminal shortcut: \(error.localizedDescription)"
            )
            shortcut = .default
        }
    }
}
