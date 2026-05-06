import Foundation

@MainActor
enum EditorTabPathMigrator {
    static func applyFileMove(
        from oldPath: String,
        to newPath: String,
        in workspaceRoots: [WorktreeKey: SplitNode]
    ) {
        guard oldPath != newPath else { return }
        let oldPrefix = oldPath + "/"
        for (_, root) in workspaceRoots {
            for area in root.allAreas() {
                for tab in area.tabs {
                    guard let editorState = tab.content.editorState else { continue }
                    let currentPath = editorState.filePath
                    if currentPath == oldPath {
                        editorState.updateFilePath(newPath)
                    } else if currentPath.hasPrefix(oldPrefix) {
                        editorState.updateFilePath(newPath + "/" + String(currentPath.dropFirst(oldPrefix.count)))
                    }
                }
            }
        }
    }
}
