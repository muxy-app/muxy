import Foundation

protocol ProjectCommandPersisting {
    func loadManualCommands() throws -> [UUID: [ProjectCommand]]
    func saveManualCommands(_ commands: [UUID: [ProjectCommand]]) throws
    func loadHiddenDiscoveredCommandIDs() throws -> [UUID: Set<String>]
    func saveHiddenDiscoveredCommandIDs(_ ids: [UUID: Set<String>]) throws
}

final class FileProjectCommandPersistence: ProjectCommandPersisting {
    private let manualStore: CodableFileStore<[UUID: [ProjectCommand]]>
    private let hiddenDiscoveredStore: CodableFileStore<[UUID: Set<String>]>

    init(
        fileURL: URL = MuxyFileStorage.fileURL(filename: "project-commands.json"),
        hiddenDiscoveredFileURL: URL = MuxyFileStorage.fileURL(filename: "hidden-project-commands.json")
    ) {
        manualStore = CodableFileStore(fileURL: fileURL, options: .pretty)
        hiddenDiscoveredStore = CodableFileStore(fileURL: hiddenDiscoveredFileURL, options: .pretty)
    }

    func loadManualCommands() throws -> [UUID: [ProjectCommand]] {
        try manualStore.load() ?? [:]
    }

    func saveManualCommands(_ commands: [UUID: [ProjectCommand]]) throws {
        try manualStore.save(commands)
    }

    func loadHiddenDiscoveredCommandIDs() throws -> [UUID: Set<String>] {
        try hiddenDiscoveredStore.load() ?? [:]
    }

    func saveHiddenDiscoveredCommandIDs(_ ids: [UUID: Set<String>]) throws {
        try hiddenDiscoveredStore.save(ids)
    }
}
