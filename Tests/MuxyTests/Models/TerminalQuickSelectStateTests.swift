import CoreGraphics
import Testing

@testable import Muxy

@MainActor
@Suite("TerminalQuickSelectState")
struct TerminalQuickSelectStateTests {
    @Test("matches common terminal tokens")
    func matchesCommonTerminalTokens() {
        let state = TerminalQuickSelectState()
        state.activate(snapshot: TerminalTextSnapshot(
            text: "open https://example.com, then ./Sources/App.swift:12 and 127.0.0.1:8080",
            viewSize: CGSize(width: 800, height: 200),
            cellSize: CGSize(width: 10, height: 20)
        ))

        #expect(state.matches.map(\.text).contains("https://example.com"))
        #expect(state.matches.map(\.text).contains("./Sources/App.swift:12"))
        #expect(state.matches.map(\.text).contains("127.0.0.1:8080"))
    }

    @Test("uses non-ambiguous labels when more than one key is required")
    func usesNonAmbiguousLabels() {
        let lines = (0 ..< 27).map { "https://example.com/\($0)" }.joined(separator: "\n")
        let state = TerminalQuickSelectState()
        state.activate(snapshot: TerminalTextSnapshot(
            text: lines,
            viewSize: CGSize(width: 800, height: 800),
            cellSize: CGSize(width: 10, height: 20)
        ))

        #expect(state.matches.count == 27)
        #expect(Set(state.matches.map(\.label)).count == 27)
        #expect(state.matches.allSatisfy { $0.label.count > 1 })
    }

    @Test("shifted final label key requests paste")
    func shiftedFinalLabelKeyRequestsPaste() {
        let lines = (0 ..< 27).map { "https://example.com/\($0)" }.joined(separator: "\n")
        let state = TerminalQuickSelectState()
        state.activate(snapshot: TerminalTextSnapshot(
            text: lines,
            viewSize: CGSize(width: 800, height: 800),
            cellSize: CGSize(width: 10, height: 20)
        ))

        let label = state.matches[0].label
        let keys = label.map(String.init)

        for key in keys.dropLast() {
            #expect(state.handle(.character(key, shifted: false)) == .none)
        }
        #expect(state.handle(.character(keys.last!, shifted: true)) == .copy("https://example.com/0", paste: true))
        #expect(!state.isVisible)
    }
}
