import Foundation

protocol WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot]
    func saveWorkspaces(_ workspaces: [WorkspaceSnapshot]) throws
}

final class FileWorkspacePersistence: WorkspacePersisting {
    private let store: CodableFileStore<[WorkspaceSnapshot]>

    init(fileURL: URL = MuxyFileStorage.fileURL(filename: "workspaces.json")) {
        store = CodableFileStore(fileURL: fileURL, options: .pretty)
    }

    func loadWorkspaces() throws -> [WorkspaceSnapshot] {
        try store.load() ?? []
    }

    func saveWorkspaces(_ workspaces: [WorkspaceSnapshot]) throws {
        try store.save(workspaces)
    }
}
