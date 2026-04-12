import Foundation

@MainActor
final class EditorViewportUndoManager {
    struct ViewportCursor {
        let line: Int
        let column: Int
    }

    struct ViewportEdit {
        let startLine: Int
        let oldLines: [String]
        let newLines: [String]
        let selectionBefore: ViewportCursor
        let selectionAfter: ViewportCursor
    }

    struct ViewportEditGroup {
        var edits: [ViewportEdit]
    }

    private static let undoLimit = 200
    private static let coalesceInterval: CFTimeInterval = 1.0

    private(set) var undoStack: [ViewportEditGroup] = []
    private(set) var redoStack: [ViewportEditGroup] = []
    private var lastEditTimestamp: CFTimeInterval?

    var canUndo: Bool { !undoStack.isEmpty }
    var canRedo: Bool { !redoStack.isEmpty }

    func clear() {
        undoStack.removeAll(keepingCapacity: false)
        redoStack.removeAll(keepingCapacity: false)
        lastEditTimestamp = nil
    }

    func push(_ edit: ViewportEdit) {
        let now = CFAbsoluteTimeGetCurrent()
        if shouldCoalesce(edit, now: now), var group = undoStack.popLast() {
            group.edits.append(edit)
            undoStack.append(group)
        } else {
            appendUndo(ViewportEditGroup(edits: [edit]))
        }
        redoStack.removeAll(keepingCapacity: false)
        lastEditTimestamp = now
    }

    func popUndo() -> ViewportEditGroup? {
        guard let group = undoStack.popLast() else { return nil }
        lastEditTimestamp = nil
        return group
    }

    func popRedo() -> ViewportEditGroup? {
        guard let group = redoStack.popLast() else { return nil }
        lastEditTimestamp = nil
        return group
    }

    func appendUndo(_ group: ViewportEditGroup) {
        undoStack.append(group)
        if undoStack.count > Self.undoLimit {
            undoStack.removeFirst(undoStack.count - Self.undoLimit)
        }
    }

    func appendRedo(_ group: ViewportEditGroup) {
        redoStack.append(group)
        if redoStack.count > Self.undoLimit {
            redoStack.removeFirst(redoStack.count - Self.undoLimit)
        }
    }

    private func shouldCoalesce(_ edit: ViewportEdit, now: CFAbsoluteTime) -> Bool {
        guard let lastTimestamp = lastEditTimestamp else { return false }
        guard now - lastTimestamp <= Self.coalesceInterval else { return false }
        guard let lastEdit = undoStack.last?.edits.last else { return false }
        return lastEdit.selectionAfter.line == edit.selectionBefore.line
            && lastEdit.selectionAfter.column == edit.selectionBefore.column
    }
}
