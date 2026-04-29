import AppKit

@MainActor
struct EditorRenderContext {
    let textView: NSTextView
    let storage: NSTextStorage
    let layoutManager: NSLayoutManager
    let viewport: ViewportState
    let backingStore: TextBackingStore
    let lineStartOffsets: [Int]
    let editorSettings: EditorSettings
    let state: EditorTabState
}

@MainActor
struct EditorTextEdit {
    let startLine: Int
    let oldLineCount: Int
    let newLineCount: Int
}
