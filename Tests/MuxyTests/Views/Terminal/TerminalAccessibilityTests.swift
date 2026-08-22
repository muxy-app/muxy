import AppKit
import Testing
@testable import Muxy

@Suite("GhosttyTerminalNSView accessibility")
@MainActor
struct TerminalAccessibilityTests {
    @Test func terminalIsExposedAsAccessibilityElement() {
        let view = GhosttyTerminalNSView(workingDirectory: "/tmp")

        #expect(view.isAccessibilityElement())
    }

    @Test func terminalReportsTextAreaRole() {
        let view = GhosttyTerminalNSView(workingDirectory: "/tmp")

        #expect(view.accessibilityRole() == .textArea)
    }

    @Test func terminalWithoutSurfaceReportsEmptyValue() {
        let view = GhosttyTerminalNSView(workingDirectory: "/tmp")

        #expect((view.accessibilityValue() as? String)?.isEmpty == true)
        #expect(view.accessibilityNumberOfCharacters() == 0)
    }

    @Test func terminalWithoutSelectionReportsNoSelectedText() {
        let view = GhosttyTerminalNSView(workingDirectory: "/tmp")

        #expect(view.accessibilitySelectedText() == nil)
    }
}
