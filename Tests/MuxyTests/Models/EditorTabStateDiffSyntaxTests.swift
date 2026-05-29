import Testing

@testable import Muxy

@Suite("EditorTabState diff syntax")
@MainActor
struct EditorTabStateDiffSyntaxTests {
    @Test("read only diff state creates syntax highlighter from file path")
    func readOnlyDiffStateCreatesSyntaxHighlighterFromFilePath() {
        let state = EditorTabState(
            projectPath: "/tmp/project",
            filePath: "/tmp/project/Sources/App.swift",
            readOnlyText: "let value = 1",
            diffLineKinds: [.addition],
            diffGutterLines: [DiffEditorGutterLine(kind: .addition, oldLineNumber: nil, newLineNumber: 1)]
        )

        #expect(state.syntaxHighlighter?.grammar.name == "Swift")
    }

    @Test("replace read only text refreshes syntax highlighter when file path changes")
    func replaceReadOnlyTextRefreshesSyntaxHighlighterWhenFilePathChanges() {
        let state = EditorTabState(
            projectPath: "/tmp/project",
            filePath: "/tmp/project/Sources/App.swift",
            readOnlyText: "let value = 1",
            diffLineKinds: [.addition]
        )

        state.replaceReadOnlyText(
            "const value = 1",
            filePath: "/tmp/project/web/app.ts",
            diffLineKinds: [.addition],
            diffGutterLines: [DiffEditorGutterLine(kind: .addition, oldLineNumber: nil, newLineNumber: 1)]
        )

        #expect(state.syntaxHighlighter?.grammar.name == "TypeScript")
    }

    @Test("read only diff state leaves syntax highlighter empty for unsupported files")
    func readOnlyDiffStateLeavesSyntaxHighlighterEmptyForUnsupportedFiles() {
        let state = EditorTabState(
            projectPath: "/tmp/project",
            filePath: "/tmp/project/file.unknownext",
            readOnlyText: "plain text",
            diffLineKinds: [.context]
        )

        #expect(state.syntaxHighlighter == nil)
    }
}
