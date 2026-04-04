import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "WorkspacePersistence")

protocol WorkspacePersisting {
    func loadWorkspaces() -> [WorkspaceSnapshot]
    func saveWorkspaces(_ workspaces: [WorkspaceSnapshot])
}

final class FileWorkspacePersistence: WorkspacePersisting {
    private let fileURL: URL

    init(fileURL: URL = MuxyFileStorage.fileURL(filename: "workspaces.json")) {
        self.fileURL = fileURL
    }

    func loadWorkspaces() -> [WorkspaceSnapshot] {
        guard FileManager.default.fileExists(atPath: fileURL.path) else { return [] }
        do {
            let data = try Data(contentsOf: fileURL)
            return try JSONDecoder().decode([WorkspaceSnapshot].self, from: data)
        } catch {
            logger.error("Failed to load workspaces: \(error)")
            return []
        }
    }

    func saveWorkspaces(_ workspaces: [WorkspaceSnapshot]) {
        do {
            let encoder = JSONEncoder()
            encoder.outputFormatting = .prettyPrinted
            let data = try encoder.encode(workspaces)
            try data.write(to: fileURL, options: .atomic)
        } catch {
            logger.error("Failed to save workspaces: \(error)")
        }
    }
}
