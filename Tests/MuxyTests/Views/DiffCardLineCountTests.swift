import AppKit
import Testing

@testable import Muxy

@Suite("DiffEditorLineMetrics")
struct DiffEditorLineMetricsTests {
    @Test("line height matches the editor's font-metric formula")
    func lineHeightMatchesEditorFormula() {
        let fontSize: CGFloat = 12
        let multiplier: CGFloat = 1.3
        let font = NSFont(name: "SF Mono", size: fontSize)
            ?? NSFont.monospacedSystemFont(ofSize: fontSize, weight: .regular)
        let expected = ceil((font.ascender - font.descender) * multiplier)

        #expect(DiffEditorLineMetrics.lineHeight(fontSize: fontSize, lineHeightMultiplier: multiplier) == expected)
    }

    @Test("editor height scales with line count and includes container insets")
    func editorHeightIncludesInsets() {
        let lineHeight = DiffEditorLineMetrics.lineHeight(fontSize: 13, lineHeightMultiplier: 1)
        let expected = lineHeight * 50 + DiffEditorLineMetrics.textContainerInset * 2

        #expect(DiffEditorLineMetrics.editorHeight(lineCount: 50, fontSize: 13, lineHeightMultiplier: 1) == expected)
    }

    @Test("line height responds to the line height multiplier")
    func lineHeightRespectsMultiplier() {
        let single = DiffEditorLineMetrics.lineHeight(fontSize: 13, lineHeightMultiplier: 1)
        let taller = DiffEditorLineMetrics.lineHeight(fontSize: 13, lineHeightMultiplier: 1.5)

        #expect(taller > single)
    }
}

@Suite("DiffCardLineCount")
struct DiffCardLineCountTests {
    @Test("uses additions and deletions before rows load")
    func usesChangeCountWhenUnloaded() {
        let section = makeSection(rows: [], additions: 120, deletions: 30)

        #expect(DiffCardLineCount.value(for: section) == 150)
    }

    @Test("uses rendered row count once rows load")
    func usesRowCountWhenLoaded() {
        let section = makeSection(rows: makeRows(count: 42), additions: 10, deletions: 5)

        #expect(DiffCardLineCount.value(for: section) == 42)
    }

    @Test("never collapses to a single placeholder line for changed files")
    func keepsHeightStableAcrossLoad() {
        let unloaded = makeSection(rows: [], additions: 200, deletions: 100)
        let loaded = makeSection(rows: makeRows(count: 320), additions: 200, deletions: 100)

        #expect(DiffCardLineCount.value(for: unloaded) == 300)
        #expect(DiffCardLineCount.value(for: loaded) == 320)
    }

    @Test("falls back to one line when nothing is known")
    func fallsBackToOneLine() {
        let section = makeSection(rows: [], additions: 0, deletions: 0)

        #expect(DiffCardLineCount.value(for: section) == 1)
    }

    private func makeSection(rows: [DiffDisplayRow], additions: Int, deletions: Int) -> DiffEditorFileSection {
        DiffEditorFileSection(
            filePath: "file.swift",
            cacheKey: "file.swift",
            rows: rows,
            isCollapsed: false,
            isLargeUnloaded: false,
            isLoading: false,
            errorMessage: nil,
            additions: additions,
            deletions: deletions,
            isStaged: false
        )
    }

    private func makeRows(count: Int) -> [DiffDisplayRow] {
        (0 ..< count).map { _ in
            DiffDisplayRow(kind: .context, oldLineNumber: nil, newLineNumber: nil, oldText: nil, newText: nil, text: "")
        }
    }
}
