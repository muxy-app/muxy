import Foundation
import Testing

@testable import Muxy

@Suite("Terminal control bytes")
struct TerminalControlBytesTests {
    @Test("single-line input is cleared with one kill")
    func killsSingleLine() {
        #expect(TerminalControlBytes.killInput(lineBreakCount: 0) == TerminalControlBytes.killLineToCursor)
    }

    @Test("each submitted line break adds a join and a kill")
    func killsEverySubmittedLine() {
        let expected = TerminalControlBytes.killLineToCursor
            + TerminalControlBytes.backspace
            + TerminalControlBytes.killLineToCursor
            + TerminalControlBytes.backspace
            + TerminalControlBytes.killLineToCursor

        #expect(TerminalControlBytes.killInput(lineBreakCount: 2) == expected)
    }

    @Test("negative line break counts fall back to a single kill")
    func ignoresNegativeLineBreakCounts() {
        #expect(TerminalControlBytes.killInput(lineBreakCount: -3) == TerminalControlBytes.killLineToCursor)
    }
}
