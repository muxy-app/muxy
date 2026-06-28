import Foundation
import Testing

@testable import Muxy

@Suite("TerminalFileOpenBehavior")
struct TerminalFileOpenBehaviorTests {
    @Test("default behavior opens with external editor")
    func defaultBehaviorUsesExternalEditor() {
        #expect(TerminalFileOpenBehavior.defaultBehavior == .externalEditor)
    }

    @Test("resolve falls back to default behavior for missing or invalid stored values")
    func resolveFallsBackToDefaultBehavior() {
        #expect(TerminalFileOpenBehavior.resolve(from: nil) == .externalEditor)
        #expect(TerminalFileOpenBehavior.resolve(from: "garbage") == .externalEditor)
    }

    @Test("resolve maps stored labels to behavior cases")
    func resolveMapsStoredLabels() {
        #expect(TerminalFileOpenBehavior.resolve(from: "Open with in-app opener only") == .inAppOpener)
        #expect(TerminalFileOpenBehavior.resolve(from: "Open in external editor") == .externalEditor)
    }

    @Test("external editor fallback is only enabled for external editor behavior")
    func externalEditorFallbackAvailability() {
        #expect(TerminalFileOpenBehavior.externalEditor.allowsExternalEditorFallback)
        #expect(!TerminalFileOpenBehavior.inAppOpener.allowsExternalEditorFallback)
    }

    @Test("all cases contains the expected picker values")
    func allCasesContainExpectedValues() {
        #expect(TerminalFileOpenBehavior.allCases == [.externalEditor, .inAppOpener])
        #expect(TerminalFileOpenBehavior.allCases.map(\.rawValue) == ["Open in external editor", "Open with in-app opener only"])
    }
}
