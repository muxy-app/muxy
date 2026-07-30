import Foundation

enum FilesDTO {
    static func entry(_ entry: FileTreeEntry) -> [String: Any] {
        [
            "name": entry.name,
            "path": entry.relativePath,
            "isDirectory": entry.isDirectory,
            "isIgnored": entry.isIgnored,
        ]
    }

    static func stat(_ result: WorkspaceFileService.StatResult) -> [String: Any] {
        [
            "name": result.name,
            "path": result.relativePath,
            "isDirectory": result.isDirectory,
            "size": result.size,
        ]
    }

    static func readResult(_ result: WorkspaceFileService.ReadResult) -> [String: Any] {
        [
            "path": result.relativePath,
            "content": result.content,
            "size": result.size,
        ]
    }
}
