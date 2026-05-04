import CoreGraphics
import Testing

@testable import Muxy

@Suite("ScrollAnchor")
@MainActor
struct ScrollAnchorTests {
    private func makeOracle(wrapping: Bool = false, lineHeight: CGFloat = 16) -> HeightOracle {
        let oracle = HeightOracle()
        oracle.updateLineHeight(lineHeight)
        oracle.updateCharWidth(8)
        oracle.updateLineLength(containerWidth: 240)
        oracle.lineWrapping = wrapping
        return oracle
    }

    @Test("pixelY for default anchor is zero")
    func defaultPixelY() {
        let oracle = makeOracle()
        let map = HeightMap(oracle: oracle)
        map.reset(lineCharCounts: [10, 10, 10])
        let anchor = ScrollAnchor()
        #expect(anchor.pixelY(in: map) == 0)
    }

    @Test("pixelY combines heightAbove with delta")
    func pixelYCombines() {
        let oracle = makeOracle()
        let map = HeightMap(oracle: oracle)
        map.reset(lineCharCounts: [10, 10, 10, 10])
        let anchor = ScrollAnchor(line: 2, deltaPixels: 4)
        #expect(anchor.pixelY(in: map) == 36)
    }

    @Test("from(pixelY:) maps to a stable anchor")
    func fromPixelYStable() {
        let oracle = makeOracle()
        let map = HeightMap(oracle: oracle)
        map.reset(lineCharCounts: [10, 10, 10, 10])
        let anchor = ScrollAnchor.from(pixelY: 36, in: map)
        #expect(anchor.line == 2)
        #expect(anchor.deltaPixels == 4)
    }

    @Test("anchor survives geometry changes that grow earlier lines")
    func anchorSurvivesGrowth() {
        let oracle = makeOracle(wrapping: true)
        let map = HeightMap(oracle: oracle)
        map.reset(lineCharCounts: [10, 10, 10, 10])
        let anchor = ScrollAnchor(line: 3, deltaPixels: 0)
        let pixelBefore = anchor.pixelY(in: map)

        map.applyMeasurements(startLine: 0, lineHeights: [80], lineCharCounts: [10])
        let pixelAfter = anchor.pixelY(in: map)

        #expect(pixelAfter > pixelBefore)
        #expect(pixelAfter == map.heightAbove(line: 3))
    }

    @Test("clamped(toLineCount:) clamps overshoot to last valid line")
    func clampedClamps() {
        let anchor = ScrollAnchor(line: 50, deltaPixels: 12)
        let clamped = anchor.clamped(toLineCount: 5)
        #expect(clamped.line == 4)
        #expect(clamped.deltaPixels == 0)
    }

    @Test("clamped(toLineCount:) returns identity when line is already valid")
    func clampedIdentity() {
        let anchor = ScrollAnchor(line: 2, deltaPixels: 8)
        let clamped = anchor.clamped(toLineCount: 5)
        #expect(clamped == anchor)
    }

    @Test("from(pixelY:) with empty map returns zero anchor")
    func fromPixelYEmptyMap() {
        let oracle = makeOracle()
        let map = HeightMap(oracle: oracle)
        let anchor = ScrollAnchor.from(pixelY: 100, in: map)
        #expect(anchor.line == 0)
        #expect(anchor.deltaPixels == 0)
    }
}
