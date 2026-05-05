import CoreGraphics
import Testing

@testable import Muxy

@Suite("HeightMap")
@MainActor
struct HeightMapTests {
    private func makeOracle(wrapping: Bool = false, lineHeight: CGFloat = 16) -> HeightOracle {
        let oracle = HeightOracle()
        oracle.updateLineHeight(lineHeight)
        oracle.updateCharWidth(8)
        oracle.updateLineLength(containerWidth: 240)
        oracle.lineWrapping = wrapping
        return oracle
    }

    @Test("reset with empty input produces empty map")
    func resetEmpty() {
        let oracle = makeOracle()
        let map = HeightMap(oracle: oracle)
        map.reset(lineCharCounts: [])
        #expect(map.totalLineCount == 0)
        #expect(map.totalHeight == 0)
        #expect(map.blocks.isEmpty)
    }

    @Test("reset with non-wrapping yields exact total height")
    func resetNonWrapping() {
        let oracle = makeOracle()
        let map = HeightMap(oracle: oracle)
        map.reset(lineCharCounts: [10, 20, 30])
        #expect(map.totalLineCount == 3)
        #expect(map.totalHeight == 48)
    }

    @Test("heightAbove sums block heights up to a line")
    func heightAboveAccumulates() {
        let oracle = makeOracle()
        let map = HeightMap(oracle: oracle)
        map.reset(lineCharCounts: [10, 10, 10, 10])
        #expect(map.heightAbove(line: 0) == 0)
        #expect(map.heightAbove(line: 1) == 16)
        #expect(map.heightAbove(line: 4) == 64)
    }

    @Test("lineAtY returns the line containing a y coordinate")
    func lineAtY() {
        let oracle = makeOracle()
        let map = HeightMap(oracle: oracle)
        map.reset(lineCharCounts: [10, 10, 10, 10])
        let location = map.lineAtY(20)
        #expect(location.line == 1)
        #expect(location.topY == 16)
        #expect(location.height == 16)
    }

    @Test("applyMeasurements decomposes a gap into measured + remaining gaps")
    func applyMeasurementsDecomposesGap() {
        let oracle = makeOracle(wrapping: true, lineHeight: 16)
        let map = HeightMap(oracle: oracle)
        map.reset(lineCharCounts: [50, 60, 70, 80, 90])
        #expect(map.blocks.count == 1)

        map.applyMeasurements(startLine: 1, lineHeights: [32, 48], lineCharCounts: [60, 70])

        #expect(map.totalLineCount == 5)
        #expect(map.blocks.count == 3)
        if case .estimated = map.blocks[0].kind { } else { Issue.record("first block should be estimated") }
        if case let .measured(heights, _) = map.blocks[1].kind {
            #expect(heights == [32, 48])
        } else {
            Issue.record("middle block should be measured")
        }
        if case .estimated = map.blocks[2].kind { } else { Issue.record("last block should be estimated") }
    }

    @Test("applyMeasurements at the start trims the leading gap")
    func applyMeasurementsAtStart() {
        let oracle = makeOracle(wrapping: true)
        let map = HeightMap(oracle: oracle)
        map.reset(lineCharCounts: [10, 20, 30])
        map.applyMeasurements(startLine: 0, lineHeights: [16, 32], lineCharCounts: [10, 20])
        #expect(map.blocks.count == 2)
        if case .measured = map.blocks[0].kind { } else { Issue.record("first block should be measured") }
        if case .estimated = map.blocks[1].kind { } else { Issue.record("trailing should be estimated") }
    }

    @Test("applyMeasurements at the end trims the trailing gap")
    func applyMeasurementsAtEnd() {
        let oracle = makeOracle(wrapping: true)
        let map = HeightMap(oracle: oracle)
        map.reset(lineCharCounts: [10, 20, 30])
        map.applyMeasurements(startLine: 1, lineHeights: [16, 32], lineCharCounts: [20, 30])
        #expect(map.blocks.count == 2)
        if case .estimated = map.blocks[0].kind { } else { Issue.record("leading should be estimated") }
        if case .measured = map.blocks[1].kind { } else { Issue.record("trailing should be measured") }
    }

    @Test("applyMeasurements covering the whole map replaces it with measured")
    func applyMeasurementsCoversAll() {
        let oracle = makeOracle(wrapping: true)
        let map = HeightMap(oracle: oracle)
        map.reset(lineCharCounts: [10, 20, 30])
        map.applyMeasurements(startLine: 0, lineHeights: [16, 16, 32], lineCharCounts: [10, 20, 30])
        #expect(map.blocks.count == 1)
        if case .measured = map.blocks[0].kind { } else { Issue.record("only block should be measured") }
        #expect(map.totalHeight == 64)
    }

    @Test("replaceLines inserts new estimated lines and shifts subsequent lines")
    func replaceLinesInsert() {
        let oracle = makeOracle()
        let map = HeightMap(oracle: oracle)
        map.reset(lineCharCounts: [10, 20])
        map.replaceLines(startLine: 1, removingCount: 0, insertingLineCharCounts: [5, 5])
        #expect(map.totalLineCount == 4)
    }

    @Test("replaceLines removes lines and shrinks total")
    func replaceLinesRemove() {
        let oracle = makeOracle()
        let map = HeightMap(oracle: oracle)
        map.reset(lineCharCounts: [10, 20, 30, 40])
        map.replaceLines(startLine: 1, removingCount: 2, insertingLineCharCounts: [])
        #expect(map.totalLineCount == 2)
        #expect(map.totalHeight == 32)
    }

    @Test("replaceLines preserves measured blocks outside the edit range")
    func replaceLinesPreservesMeasured() {
        let oracle = makeOracle(wrapping: true)
        let map = HeightMap(oracle: oracle)
        map.reset(lineCharCounts: [10, 20, 30, 40])
        map.applyMeasurements(startLine: 0, lineHeights: [16], lineCharCounts: [10])
        map.applyMeasurements(startLine: 3, lineHeights: [32], lineCharCounts: [40])
        map.replaceLines(startLine: 1, removingCount: 2, insertingLineCharCounts: [5])
        #expect(map.totalLineCount == 3)
        if case .measured = map.blocks.first?.kind { } else { Issue.record("first block should remain measured") }
        if case .measured = map.blocks.last?.kind { } else { Issue.record("last block should remain measured") }
    }

    @Test("reset after measurements collapses back to a single gap")
    func resetAfterMeasurements() {
        let oracle = makeOracle(wrapping: true)
        let map = HeightMap(oracle: oracle)
        map.reset(lineCharCounts: [10, 20, 30])
        map.applyMeasurements(startLine: 0, lineHeights: [16, 32, 48], lineCharCounts: [10, 20, 30])
        map.reset(lineCharCounts: [10, 20, 30, 40])
        #expect(map.totalLineCount == 4)
        #expect(map.blocks.count == 1)
        if case .estimated = map.blocks[0].kind { } else { Issue.record("after reset should be estimated") }
    }

    @Test("heightAbove last line equals totalHeight minus last line height")
    func heightAboveLast() {
        let oracle = makeOracle(wrapping: true)
        let map = HeightMap(oracle: oracle)
        map.reset(lineCharCounts: [10, 20, 30])
        map.applyMeasurements(startLine: 0, lineHeights: [16, 32, 48], lineCharCounts: [10, 20, 30])
        let aboveLast = map.heightAbove(line: 2)
        let lastHeight = map.heightOfLine(2)
        #expect(aboveLast + lastHeight == map.totalHeight)
    }

    @Test("estimated gap height scales with character density")
    func estimatedGapDensity() {
        let oracle = makeOracle(wrapping: true)
        let sparse = HeightMap(oracle: oracle)
        sparse.reset(lineCharCounts: Array(repeating: 5, count: 100))
        let dense = HeightMap(oracle: oracle)
        dense.reset(lineCharCounts: Array(repeating: 200, count: 100))
        #expect(dense.totalHeight > sparse.totalHeight)
    }

    @Test("wrapped estimated lines keep exact per-line coordinates")
    func wrappedEstimatedLinesUsePerLinePrefixes() {
        let oracle = makeOracle(wrapping: true, lineHeight: 10)
        let map = HeightMap(oracle: oracle)
        map.reset(lineCharCounts: [5, 65, 10])

        #expect(map.heightAbove(line: 0) == 0)
        #expect(map.heightAbove(line: 1) == 10)
        #expect(map.heightAbove(line: 2) == 40)
        #expect(map.heightAbove(line: 3) == 50)

        let location = map.lineAtY(35)
        #expect(location.line == 1)
        #expect(location.topY == 10)
        #expect(location.height == 30)
    }
}
