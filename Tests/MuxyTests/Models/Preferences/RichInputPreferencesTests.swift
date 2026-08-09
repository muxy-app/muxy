import Testing

@testable import Muxy

@Suite("Rich input preferences")
struct RichInputPreferencesTests {
    @Test("panel presentation is the default")
    func panelPresentationIsDefault() {
        #expect(RichInputPreferences.defaultPresentationMode == .panel)
        #expect(RichInputPreferences.defaultPosition == .right)
    }

    @Test("presentation modes have stable persisted values")
    func presentationModesHaveStablePersistedValues() {
        #expect(RichInputPresentationMode.allCases.map(\.rawValue) == ["panel", "floating"])
        #expect(RichInputPresentationMode.allCases.map(\.displayName) == ["Panel", "Floating"])
        #expect(RichInputPresentationMode(rawValue: "panel") == .panel)
        #expect(RichInputPresentationMode(rawValue: "floating") == .floating)
    }
}
