import Testing

@testable import Muxy

@Suite("NotchTerminalDimensionInput")
struct NotchTerminalDimensionInputTests {
    @Test("external reset replaces an uncommitted draft")
    func resetReplacesDraft() {
        var input = NotchTerminalDimensionInput(text: "960")

        input.synchronize(with: NotchTerminalSizePreferences.defaultWidth)

        #expect(input.text == "720")
        #expect(input.commit(currentValue: 720, range: NotchTerminalSizePreferences.widthRange) == 720)
    }

    @Test("commit clamps valid values and restores invalid drafts")
    func commitValidation() {
        var input = NotchTerminalDimensionInput(text: "2000")

        #expect(input.commit(currentValue: 720, range: NotchTerminalSizePreferences.widthRange) == 1_200)
        #expect(input.text == "1200")

        input.text = "invalid"

        #expect(input.commit(currentValue: 1_200, range: NotchTerminalSizePreferences.widthRange) == 1_200)
        #expect(input.text == "1200")
    }
}
