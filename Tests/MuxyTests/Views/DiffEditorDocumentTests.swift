import Testing

@testable import Muxy

@Suite("DiffEditorDocument")
struct DiffEditorDocumentTests {
    @Test("unified document keeps line columns out of selectable text")
    func unifiedDocumentKeepsLineColumnsOutOfSelectableText() {
        let rows = [
            DiffDisplayRow(kind: .context, oldLineNumber: 1, newLineNumber: 1, oldText: "let a = 1", newText: "let a = 1", text: " let a = 1"),
            DiffDisplayRow(kind: .deletion, oldLineNumber: 2, newLineNumber: nil, oldText: "let b = 2", newText: nil, text: "-let b = 2"),
            DiffDisplayRow(kind: .addition, oldLineNumber: nil, newLineNumber: 2, oldText: nil, newText: "let b = 3", text: "+let b = 3"),
        ]

        let document = DiffEditorDocument.unified(rows: rows)

        #expect(document.text == "let a = 1\nlet b = 2\nlet b = 3")
        #expect(document.lineKinds.count == rows.count)
        #expect(document.gutterLines.count == rows.count)
        #expect(document.gutterLines[0].oldLineNumber == 1)
        #expect(document.gutterLines[0].newLineNumber == 1)
        #expect(document.gutterLines[1].kind == .deletion)
        #expect(document.gutterLines[1].oldLineNumber == 2)
        #expect(document.gutterLines[2].kind == .addition)
        #expect(document.gutterLines[2].newLineNumber == 2)
    }

    @Test("split documents preserve paired row counts")
    func splitDocumentsPreservePairedRowCounts() {
        let rows = [
            DiffDisplayRow(kind: .deletion, oldLineNumber: 10, newLineNumber: nil, oldText: "old", newText: nil, text: "-old"),
            DiffDisplayRow(kind: .addition, oldLineNumber: nil, newLineNumber: 10, oldText: nil, newText: "new", text: "+new"),
            DiffDisplayRow(kind: .addition, oldLineNumber: nil, newLineNumber: 11, oldText: nil, newText: "extra", text: "+extra"),
        ]

        let left = DiffEditorDocument.splitLeft(rows: rows)
        let right = DiffEditorDocument.splitRight(rows: rows)

        #expect(left.text.split(separator: "\n", omittingEmptySubsequences: false).count == 2)
        #expect(right.text.split(separator: "\n", omittingEmptySubsequences: false).count == 2)
        #expect(left.text == "old\n")
        #expect(right.text == "new\nextra")
        #expect(left.gutterLines[0].kind == .deletion)
        #expect(left.gutterLines[0].oldLineNumber == 10)
        #expect(left.gutterLines[1] == DiffEditorGutterLine(kind: .context, oldLineNumber: nil, newLineNumber: nil))
        #expect(right.gutterLines[0].kind == .addition)
        #expect(right.gutterLines[0].newLineNumber == 10)
        #expect(right.gutterLines[1].kind == .addition)
        #expect(right.gutterLines[1].newLineNumber == 11)
    }

    @Test("combined document records file section line indexes")
    func combinedDocumentRecordsFileSectionLineIndexes() {
        let firstRows = [
            DiffDisplayRow(kind: .addition, oldLineNumber: nil, newLineNumber: 1, oldText: nil, newText: "first", text: "+first"),
        ]
        let secondRows = [
            DiffDisplayRow(kind: .deletion, oldLineNumber: 4, newLineNumber: nil, oldText: "second", newText: nil, text: "-second"),
        ]

        let document = DiffEditorDocument.unified(sections: [
            DiffEditorFileSection(
                filePath: "Sources/First.swift",
                cacheKey: "unstaged:Sources/First.swift",
                rows: firstRows,
                isCollapsed: false,
                isLargeUnloaded: false,
                additions: 1,
                deletions: 0,
                isStaged: false
            ),
            DiffEditorFileSection(
                filePath: "Sources/Second.swift",
                cacheKey: "staged:Sources/Second.swift",
                rows: secondRows,
                isCollapsed: false,
                isLargeUnloaded: false,
                additions: 0,
                deletions: 1,
                isStaged: true
            ),
        ])

        #expect(document.text == "▾ Sources/First.swift +1\nfirst\n\n▾ Sources/Second.swift Staged -1\nsecond")
        #expect(document.fileLineIndexes["unstaged:Sources/First.swift"] == 0)
        #expect(document.fileLineIndexes["staged:Sources/Second.swift"] == 3)
        #expect(document.lineKinds[0] == .hunk)
        #expect(document.lineKinds[3] == .hunk)
        #expect(document.gutterLines[0] == DiffEditorGutterLine(kind: .hunk, oldLineNumber: nil, newLineNumber: nil))
        #expect(document.gutterLines[3] == DiffEditorGutterLine(kind: .hunk, oldLineNumber: nil, newLineNumber: nil))
    }

    @Test("combined document omits collapsed section rows")
    func combinedDocumentOmitsCollapsedSectionRows() {
        let document = DiffEditorDocument.unified(sections: [
            DiffEditorFileSection(
                filePath: "Sources/App.swift",
                cacheKey: "unstaged:Sources/App.swift",
                rows: [DiffDisplayRow(kind: .addition, oldLineNumber: nil, newLineNumber: 1, oldText: nil, newText: "hidden", text: "+hidden")],
                isCollapsed: true,
                isLargeUnloaded: false,
                additions: 1,
                deletions: 0,
                isStaged: false
            ),
        ])

        #expect(document.text == "▸ Sources/App.swift +1")
        #expect(document.lineKinds == [.hunk])
        #expect(document.gutterLines[0] == DiffEditorGutterLine(kind: .hunk, oldLineNumber: nil, newLineNumber: nil))
    }

    @Test("long diff lines are clipped for rendering")
    func longDiffLinesAreClippedForRendering() {
        let row = DiffDisplayRow(
            kind: .addition,
            oldLineNumber: nil,
            newLineNumber: 1,
            oldText: nil,
            newText: String(repeating: "a", count: 20),
            text: "+"
        )

        let document = DiffEditorDocument.unified(
            rows: [row],
            options: DiffEditorDocument.RenderOptions(maxLineCharacters: 8)
        )

        #expect(document.text == "aaaaaaaa … [12 chars clipped]")
        #expect(document.lineKinds == [.addition])
        #expect(document.gutterLines[0].newLineNumber == 1)
    }
}
