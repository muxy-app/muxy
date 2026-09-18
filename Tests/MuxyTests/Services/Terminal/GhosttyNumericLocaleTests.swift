import Darwin
import Foundation
import Testing

@testable import Muxy

@Suite("Ghostty numeric locale")
struct GhosttyNumericLocaleTests {
    @MainActor
    @Test("keeps the C numeric locale that AppKit requires after Ghostty initialization")
    func numericLocaleStaysPOSIX() throws {
        _ = GhosttyService.shared

        let numericLocale = try #require(setlocale(LC_NUMERIC, nil))

        #expect(String(cString: numericLocale) == "C")
    }
}
