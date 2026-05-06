import Testing

@testable import Muxy

@Suite("HeightOracle")
@MainActor
struct HeightOracleTests {
    @Test("non-wrapping returns one lineHeight per logical line")
    func nonWrapping() {
        let oracle = HeightOracle()
        oracle.updateLineHeight(20)
        oracle.lineWrapping = false
        #expect(oracle.heightForLine(charCount: 0) == 20)
        #expect(oracle.heightForLine(charCount: 5_000) == 20)
    }

    @Test("wrapping short line returns single row height")
    func wrappingShortLine() {
        let oracle = HeightOracle()
        oracle.updateLineHeight(16)
        oracle.updateCharWidth(8)
        oracle.updateLineLength(containerWidth: 240)
        oracle.lineWrapping = true
        #expect(oracle.lineLength == 30)
        #expect(oracle.heightForLine(charCount: 10) == 16)
        #expect(oracle.heightForLine(charCount: 30) == 16)
    }

    @Test("wrapping long line scales with character count")
    func wrappingLongLine() {
        let oracle = HeightOracle()
        oracle.updateLineHeight(16)
        oracle.updateCharWidth(8)
        oracle.updateLineLength(containerWidth: 240)
        oracle.lineWrapping = true
        let height120 = oracle.heightForLine(charCount: 120)
        let height500 = oracle.heightForLine(charCount: 500)
        #expect(height120 > 16)
        #expect(height500 > height120)
    }

    @Test("updateLineLength clamps to reasonable minimum")
    func updateLineLengthMinimum() {
        let oracle = HeightOracle()
        oracle.updateCharWidth(8)
        oracle.updateLineLength(containerWidth: 8)
        #expect(oracle.lineLength >= 5)
    }

    @Test("invalid updates are ignored")
    func invalidUpdates() {
        let oracle = HeightOracle()
        oracle.updateLineHeight(0)
        oracle.updateLineHeight(-1)
        oracle.updateCharWidth(0)
        oracle.updateCharWidth(-1)
        oracle.updateLineLength(containerWidth: 0)
        #expect(oracle.lineHeight > 0)
        #expect(oracle.charWidth > 0)
        #expect(oracle.lineLength > 0)
    }
}
