import AppKit
import Testing
@testable import Muxy

@Suite("GhosttyTerminalNSView NSTextInputClient")
@MainActor
struct TerminalTextInputClientTests {
    @Test func initialUnfocusedSurfaceDoesNotNeedNativeFocusUpdate() {
        #expect(GhosttyTerminalNSView.shouldApplySurfaceFocusChange(previous: nil, next: false) == false)
    }

    @Test func focusTransitionsNeedNativeFocusUpdates() {
        #expect(GhosttyTerminalNSView.shouldApplySurfaceFocusChange(previous: nil, next: true))
        #expect(GhosttyTerminalNSView.shouldApplySurfaceFocusChange(previous: true, next: false))
        #expect(GhosttyTerminalNSView.shouldApplySurfaceFocusChange(previous: false, next: true))
    }

    @Test func duplicateFocusStateDoesNotNeedNativeFocusUpdate() {
        #expect(GhosttyTerminalNSView.shouldApplySurfaceFocusChange(previous: true, next: true) == false)
        #expect(GhosttyTerminalNSView.shouldApplySurfaceFocusChange(previous: false, next: false) == false)
    }

    @Test func selectedRangeDefaultsToValidInsertionPoint() {
        let view = GhosttyTerminalNSView(workingDirectory: "/tmp")

        #expect(view.selectedRange() == NSRange(location: 0, length: 0))
    }

    @Test func markedRangeUsesUTF16LengthAndClampsSelection() {
        let view = GhosttyTerminalNSView(workingDirectory: "/tmp")

        view.setMarkedText("👋a", selectedRange: NSRange(location: 10, length: 2), replacementRange: NSRange(location: NSNotFound, length: 0))

        #expect(view.markedRange() == NSRange(location: 0, length: 3))
        #expect(view.selectedRange() == NSRange(location: 3, length: 0))
    }

    @Test func attributedSubstringReturnsVirtualMarkedText() throws {
        let view = GhosttyTerminalNSView(workingDirectory: "/tmp")
        view.setMarkedText("compose", selectedRange: NSRange(location: 0, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        var actualRange = NSRange(location: NSNotFound, length: 0)

        let substring = try #require(view.attributedSubstring(forProposedRange: NSRange(location: 1, length: 3), actualRange: &actualRange))

        #expect(substring.string == "omp")
        #expect(actualRange == NSRange(location: 1, length: 3))
    }
}
