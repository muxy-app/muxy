import Foundation

@MainActor
@Observable
final class WorkspaceFileWatcher {
    private var rootPath: String?
    @ObservationIgnored private var watcher: FileSystemWatcher?

    func setRoot(_ path: String?) {
        guard path != rootPath else { return }
        rootPath = path
        watcher = nil
        guard let path else { return }
        watcher = FileSystemWatcher(directoryPath: path) { changedPaths in
            ExtensionFileEventEmitter.emit(paths: changedPaths, projectPath: path)
        }
    }
}
