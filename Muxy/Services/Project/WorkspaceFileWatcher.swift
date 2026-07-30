import Foundation

@MainActor
@Observable
final class WorkspaceFileWatcher {
    struct Root: Equatable, Sendable {
        let projectID: UUID
        let worktreeID: UUID?
        let path: String
    }

    private var root: Root?
    @ObservationIgnored private var watcher: FileSystemWatcher?

    func setRoot(_ root: Root?) {
        guard root != self.root else { return }
        self.root = root
        watcher = nil
        guard let root else { return }
        watcher = FileSystemWatcher(directoryPath: root.path) { changedPaths in
            ExtensionFileEventEmitter.emit(paths: changedPaths, projectPath: root.path)
            WorkspaceFilesChange(
                projectID: root.projectID,
                worktreeID: root.worktreeID,
                root: root.path,
                paths: changedPaths
            ).post()
        }
    }
}
