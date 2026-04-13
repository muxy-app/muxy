import Testing

@testable import Muxy

@Suite("GitDiffParser")
struct GitDiffParserTests {
    @Test("parseRows with empty string returns empty result")
    func parseRowsEmpty() {
        let result = GitDiffParser.parseRows("")
        #expect(result.rows.isEmpty)
        #expect(result.additions == 0)
        #expect(result.deletions == 0)
    }

    @Test("parseRows skips lines before first hunk")
    func parseRowsSkipsPreHunkLines() {
        let patch = """
        diff --git a/file.swift b/file.swift
        index abc..def 100644
        --- a/file.swift
        +++ b/file.swift
        @@ -1,3 +1,3 @@
         context
        -old
        +new
        """
        let result = GitDiffParser.parseRows(patch)
        #expect(result.rows.count == 4)
        #expect(result.rows[0].kind == .hunk)
    }

    @Test("parseRows with single hunk parses all row kinds")
    func parseRowsSingleHunk() {
        let patch = """
        @@ -10,4 +10,4 @@
         context line
        -deleted line
        +added line
         more context
        """
        let result = GitDiffParser.parseRows(patch)

        #expect(result.rows.count == 5)
        #expect(result.rows[0].kind == .hunk)
        #expect(result.rows[1].kind == .context)
        #expect(result.rows[1].oldLineNumber == 10)
        #expect(result.rows[1].newLineNumber == 10)
        #expect(result.rows[2].kind == .deletion)
        #expect(result.rows[2].oldLineNumber == 11)
        #expect(result.rows[2].newLineNumber == nil)
        #expect(result.rows[3].kind == .addition)
        #expect(result.rows[3].oldLineNumber == nil)
        #expect(result.rows[3].newLineNumber == 11)
        #expect(result.rows[4].kind == .context)
        #expect(result.rows[4].oldLineNumber == 12)
        #expect(result.rows[4].newLineNumber == 12)
        #expect(result.additions == 1)
        #expect(result.deletions == 1)
    }

    @Test("parseRows additions only")
    func parseRowsAdditionsOnly() {
        let patch = """
        @@ -5,2 +5,4 @@
         existing
        +new1
        +new2
         existing2
        """
        let result = GitDiffParser.parseRows(patch)
        #expect(result.additions == 2)
        #expect(result.deletions == 0)

        let additions = result.rows.filter { $0.kind == .addition }
        #expect(additions.count == 2)
        #expect(additions[0].newLineNumber == 6)
        #expect(additions[1].newLineNumber == 7)
    }

    @Test("parseRows deletions only")
    func parseRowsDeletionsOnly() {
        let patch = """
        @@ -5,4 +5,2 @@
         existing
        -removed1
        -removed2
         existing2
        """
        let result = GitDiffParser.parseRows(patch)
        #expect(result.additions == 0)
        #expect(result.deletions == 2)

        let deletions = result.rows.filter { $0.kind == .deletion }
        #expect(deletions.count == 2)
        #expect(deletions[0].oldLineNumber == 6)
        #expect(deletions[1].oldLineNumber == 7)
    }

    @Test("parseRows with multiple hunks resets line numbers")
    func parseRowsMultipleHunks() {
        let patch = """
        @@ -1,3 +1,3 @@
         a
        -b
        +c
        @@ -20,3 +20,3 @@
         x
        -y
        +z
        """
        let result = GitDiffParser.parseRows(patch)

        let hunks = result.rows.filter { $0.kind == .hunk }
        #expect(hunks.count == 2)

        let secondDeletion = result.rows.filter { $0.kind == .deletion }[1]
        #expect(secondDeletion.oldLineNumber == 21)

        let secondAddition = result.rows.filter { $0.kind == .addition }[1]
        #expect(secondAddition.newLineNumber == 21)
    }

    @Test("parseRows captures text content correctly")
    func parseRowsTextContent() {
        let patch = """
        @@ -1,3 +1,3 @@
         context
        -old
        +new
        """
        let result = GitDiffParser.parseRows(patch)

        #expect(result.rows[1].text == " context")
        #expect(result.rows[1].oldText == "context")
        #expect(result.rows[1].newText == "context")
        #expect(result.rows[2].text == "-old")
        #expect(result.rows[2].oldText == "old")
        #expect(result.rows[2].newText == nil)
        #expect(result.rows[3].text == "+new")
        #expect(result.rows[3].oldText == nil)
        #expect(result.rows[3].newText == "new")
    }

    @Test("collapseContextRows preserves short runs")
    func collapseShortRun() {
        var rows: [DiffDisplayRow] = []
        for i in 0 ..< 12 {
            rows.append(DiffDisplayRow(
                kind: .context,
                oldLineNumber: i,
                newLineNumber: i,
                oldText: "line \(i)",
                newText: "line \(i)",
                text: " line \(i)"
            ))
        }
        let collapsed = GitDiffParser.collapseContextRows(rows)
        #expect(collapsed.count == 12)
        #expect(collapsed.allSatisfy { $0.kind == .context })
    }

    @Test("collapseContextRows collapses long runs")
    func collapseLongRun() {
        var rows: [DiffDisplayRow] = []
        for i in 0 ..< 20 {
            rows.append(DiffDisplayRow(
                kind: .context,
                oldLineNumber: i,
                newLineNumber: i,
                oldText: "line \(i)",
                newText: "line \(i)",
                text: " line \(i)"
            ))
        }
        let collapsed = GitDiffParser.collapseContextRows(rows)

        #expect(collapsed.count == 7)
        #expect(collapsed[0].kind == .context)
        #expect(collapsed[1].kind == .context)
        #expect(collapsed[2].kind == .context)
        #expect(collapsed[3].kind == .collapsed)
        #expect(collapsed[3].text == "14 unmodified lines")
        #expect(collapsed[4].kind == .context)
        #expect(collapsed[5].kind == .context)
        #expect(collapsed[6].kind == .context)
    }

    @Test("collapseContextRows preserves non-context rows")
    func collapsePreservesNonContext() {
        let rows: [DiffDisplayRow] = [
            DiffDisplayRow(kind: .hunk, oldLineNumber: nil, newLineNumber: nil, oldText: nil, newText: nil, text: "@@ -1,1 +1,1 @@"),
            DiffDisplayRow(kind: .deletion, oldLineNumber: 1, newLineNumber: nil, oldText: "old", newText: nil, text: "-old"),
            DiffDisplayRow(kind: .addition, oldLineNumber: nil, newLineNumber: 1, oldText: nil, newText: "new", text: "+new"),
        ]
        let collapsed = GitDiffParser.collapseContextRows(rows)
        #expect(collapsed.count == 3)
    }

    @Test("collapseContextRows handles mixed context and changes")
    func collapseMixedContent() {
        var rows: [DiffDisplayRow] = []
        for i in 0 ..< 20 {
            rows.append(DiffDisplayRow(kind: .context, oldLineNumber: i, newLineNumber: i, oldText: "\(i)", newText: "\(i)", text: " \(i)"))
        }
        rows.append(DiffDisplayRow(kind: .deletion, oldLineNumber: 20, newLineNumber: nil, oldText: "x", newText: nil, text: "-x"))
        for i in 20 ..< 40 {
            rows.append(DiffDisplayRow(kind: .context, oldLineNumber: i, newLineNumber: i, oldText: "\(i)", newText: "\(i)", text: " \(i)"))
        }

        let collapsed = GitDiffParser.collapseContextRows(rows)
        let collapsedMarkers = collapsed.filter { $0.kind == .collapsed }
        #expect(collapsedMarkers.count == 2)
    }

    @Test("parseHunkHeader standard format")
    func parseHunkHeaderStandard() {
        let (old, new) = GitDiffParser.parseHunkHeader("@@ -10,5 +20,8 @@")
        #expect(old == 10)
        #expect(new == 20)
    }

    @Test("parseHunkHeader single line")
    func parseHunkHeaderSingleLine() {
        let (old, new) = GitDiffParser.parseHunkHeader("@@ -1 +1 @@")
        #expect(old == 1)
        #expect(new == 1)
    }

    @Test("parseHunkHeader malformed returns zeros")
    func parseHunkHeaderMalformed() {
        let (old, new) = GitDiffParser.parseHunkHeader("@@")
        #expect(old == 0)
        #expect(new == 0)
    }

    @Test("parseHunkNumber with comma")
    func parseHunkNumberWithComma() {
        #expect(GitDiffParser.parseHunkNumber("-10,5") == 10)
    }

    @Test("parseHunkNumber without comma")
    func parseHunkNumberWithoutComma() {
        #expect(GitDiffParser.parseHunkNumber("+3") == 3)
    }

    @Test("parseHunkNumber empty string returns zero")
    func parseHunkNumberEmpty() {
        #expect(GitDiffParser.parseHunkNumber("") == 0)
    }
}
