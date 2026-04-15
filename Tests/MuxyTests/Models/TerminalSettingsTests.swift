import Foundation
import Testing

@testable import Muxy

@Suite("TerminalSettings")
struct TerminalSettingsTests {
    @Test("custom labels are lowercased letters with duplicates removed")
    func customLabelsAreSanitized() {
        #expect(TerminalSettings.sanitizeQuickSelectLabelString("AARs-12T") == "arst")
    }

    @Test("custom labels use only configured keys")
    func customLabelsUseOnlyConfiguredKeys() {
        let labels = TerminalSettings.quickSelectLabels(for: "arst")
        #expect(labels == ["a", "r", "s", "t"])
    }

    @Test("empty custom labels fall back to qwerty home row")
    func emptyCustomLabelsFallBackToQwertyHomeRow() {
        #expect(TerminalSettings.quickSelectLabels(for: "").joined() == "asdfghjkl")
    }

    @Test("built-in layouts use home row keys")
    func builtInLayoutsUseHomeRowKeys() {
        #expect(QuickSelectKeyboardLayout.qwerty.defaultLabels == "asdfghjkl")
        #expect(QuickSelectKeyboardLayout.colemak.defaultLabels == "arstdhneio")
        #expect(QuickSelectKeyboardLayout.colemakDH.defaultLabels == "arstgmneio")
        #expect(QuickSelectKeyboardLayout.dvorak.defaultLabels == "aoeuidhtns")
        #expect(QuickSelectKeyboardLayout.workman.defaultLabels == "ashtgyneoi")
    }
}
