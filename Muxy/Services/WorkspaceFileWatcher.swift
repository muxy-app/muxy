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
            let gitPaths = changedPaths.filter { $0.contains("/.git/") }
            let filePaths = changedPaths.filter { !$0.contains("/.git/") }
            if !gitPaths.isEmpty {
                ExtensionGitEventEmitter.emit(projectPath: path)
            }
            ExtensionFileEventEmitter.emit(paths: filePaths, projectPath: path)
        }
    }
}
