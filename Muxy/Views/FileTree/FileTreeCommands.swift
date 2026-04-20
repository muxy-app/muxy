import AppKit
import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "FileTreeCommands")

@MainActor
@Observable
final class FileTreeCommands {
    private let state: FileTreeState
    private let openTerminal: (String) -> Void
    private let onFileMoved: (String, String) -> Void

    init(
        state: FileTreeState,
        openTerminal: @escaping (String) -> Void,
        onFileMoved: @escaping (String, String) -> Void
    ) {
        self.state = state
        self.openTerminal = openTerminal
        self.onFileMoved = onFileMoved
    }

    func beginNewFile(in directoryPath: String) {
        let parent = resolveDirectoryContext(for: directoryPath)
        state.expand(path: parent)
        state.pendingNewEntry = FileTreeState.PendingNewEntry(parentPath: parent, kind: .file)
        state.pendingRenamePath = nil
    }

    func beginNewFolder(in directoryPath: String) {
        let parent = resolveDirectoryContext(for: directoryPath)
        state.expand(path: parent)
        state.pendingNewEntry = FileTreeState.PendingNewEntry(parentPath: parent, kind: .folder)
        state.pendingRenamePath = nil
    }

    func commitNewEntry(name: String) {
        guard let pending = state.pendingNewEntry else { return }
        state.pendingNewEntry = nil
        Task { [parent = pending.parentPath, kind = pending.kind] in
            do {
                let created = switch kind {
                case .file: try await FileSystemOperations.createFile(named: name, in: parent)
                case .folder: try await FileSystemOperations.createFolder(named: name, in: parent)
                }
                state.refreshDirectory(path: parent)
                if kind == .file {
                    state.selectedFilePath = created
                }
            } catch {
                let label = kind == .file ? "file" : "folder"
                logger.error("Create \(label, privacy: .public) failed: \(error.localizedDescription, privacy: .public)")
            }
        }
    }

    func cancelNewEntry() {
        state.pendingNewEntry = nil
    }

    func beginRename(path: String) {
        state.pendingRenamePath = path
        state.pendingNewEntry = nil
    }

    func commitRename(originalPath: String, newName: String) {
        state.pendingRenamePath = nil
        let parent = state.parentDirectory(of: originalPath)
        Task {
            do {
                let newPath = try await FileSystemOperations.rename(at: originalPath, to: newName)
                state.refreshDirectory(path: parent)
                if state.selectedFilePath == originalPath {
                    state.selectedFilePath = newPath
                }
                onFileMoved(originalPath, newPath)
            } catch {
                logger.error("Rename failed: \(error.localizedDescription, privacy: .public)")
            }
        }
    }

    func cancelRename() {
        state.pendingRenamePath = nil
    }

    func trash(path: String) {
        state.pendingDeletePath = path
    }

    func cancelPendingDelete() {
        state.pendingDeletePath = nil
    }

    func confirmPendingDelete() {
        guard let path = state.pendingDeletePath else { return }
        state.pendingDeletePath = nil
        let parent = state.parentDirectory(of: path)
        Task {
            do {
                try await FileSystemOperations.moveToTrash([path])
                state.refreshDirectory(path: parent)
                if state.selectedFilePath == path {
                    state.selectedFilePath = nil
                }
            } catch {
                logger.error("Trash failed: \(error.localizedDescription, privacy: .public)")
            }
        }
    }

    func copyToClipboard(paths: [String]) {
        FileClipboard.write(paths: paths, isCut: false)
        state.cutPaths = []
    }

    func cutToClipboard(paths: [String]) {
        FileClipboard.write(paths: paths, isCut: true)
        state.cutPaths = Set(paths)
    }

    func paste(into destinationPath: String) {
        guard let contents = FileClipboard.read() else { return }
        let destination = resolveDirectoryContext(for: destinationPath)
        Task { [paths = contents.paths, isCut = contents.isCut] in
            do {
                let results: [String] = if isCut {
                    try await FileSystemOperations.move(paths, into: destination)
                } else {
                    try await FileSystemOperations.copy(paths, into: destination)
                }
                state.refreshDirectory(path: destination)
                for source in paths {
                    let sourceParent = state.parentDirectory(of: source)
                    if sourceParent != destination {
                        state.refreshDirectory(path: sourceParent)
                    }
                }
                if isCut {
                    state.cutPaths = []
                    FileClipboard.clearCutMarker()
                    for (index, source) in paths.enumerated() where index < results.count {
                        onFileMoved(source, results[index])
                    }
                }
            } catch {
                logger.error("Paste failed: \(error.localizedDescription, privacy: .public)")
            }
        }
    }

    func copyAbsolutePath(_ path: String) {
        let pb = NSPasteboard.general
        pb.clearContents()
        pb.setString(path, forType: .string)
    }

    func copyRelativePath(_ path: String) {
        let root = state.rootPath.hasSuffix("/") ? String(state.rootPath.dropLast()) : state.rootPath
        let relative: String = if path.hasPrefix(root + "/") {
            String(path.dropFirst(root.count + 1))
        } else {
            (path as NSString).lastPathComponent
        }
        let pb = NSPasteboard.general
        pb.clearContents()
        pb.setString(relative, forType: .string)
    }

    func revealInFinder(_ path: String) {
        let url = URL(fileURLWithPath: path)
        NSWorkspace.shared.activateFileViewerSelecting([url])
    }

    func openInTerminal(path: String) {
        let dir = resolveDirectoryContext(for: path)
        openTerminal(dir)
    }

    func performDrop(sources: [String], destinationPath: String, copy: Bool) {
        let destination = resolveDirectoryContext(for: destinationPath)
        Task {
            do {
                let results: [String] = if copy {
                    try await FileSystemOperations.copy(sources, into: destination)
                } else {
                    try await FileSystemOperations.move(sources, into: destination)
                }
                state.refreshDirectory(path: destination)
                for source in sources {
                    let sourceParent = state.parentDirectory(of: source)
                    if sourceParent != destination {
                        state.refreshDirectory(path: sourceParent)
                    }
                }
                if !copy {
                    for (index, source) in sources.enumerated() where index < results.count {
                        onFileMoved(source, results[index])
                    }
                }
            } catch {
                logger.error("Drop failed: \(error.localizedDescription, privacy: .public)")
            }
        }
    }

    private func resolveDirectoryContext(for path: String) -> String {
        let normalized = path.hasSuffix("/") ? String(path.dropLast()) : path
        var isDir: ObjCBool = false
        if FileManager.default.fileExists(atPath: normalized, isDirectory: &isDir), isDir.boolValue {
            return normalized
        }
        return (normalized as NSString).deletingLastPathComponent
    }
}
