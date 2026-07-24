import Testing

@testable import Muxy

@Suite("QuickTerminalDimensionInput")
struct QuickTerminalDimensionInputTests {
    @Test("external reset replaces an uncommitted draft")
    func resetReplacesDraft() {
        var input = QuickTerminalDimensionInput(text: "960")

        input.synchronize(with: QuickTerminalSizePreferences.defaultWidth)

        #expect(input.text == "720")
        #expect(input.commit(currentValue: 720, range: QuickTerminalSizePreferences.widthRange) == 720)
    }

    @Test("commit clamps valid values and restores invalid drafts")
    func commitValidation() {
        var input = QuickTerminalDimensionInput(text: "2000")

        #expect(input.commit(currentValue: 720, range: QuickTerminalSizePreferences.widthRange) == 1_200)
        #expect(input.text == "1200")

        input.text = "invalid"

        #expect(input.commit(currentValue: 1_200, range: QuickTerminalSizePreferences.widthRange) == 1_200)
        #expect(input.text == "1200")
    }
}
