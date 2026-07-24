import Testing

@testable import Muxy

@Suite("Quick terminal focus restoration policy")
struct QuickTerminalFocusRestorationPolicyTests {
    @Test("captures an initial focus snapshot")
    func capturesInitialSnapshot() {
        #expect(QuickTerminalFocusRestorationPolicy.shouldCapture(hasSnapshot: false, panelIsKey: false))
    }

    @Test("keeps the snapshot while the panel owns focus")
    func keepsSnapshotWhilePanelIsKey() {
        #expect(!QuickTerminalFocusRestorationPolicy.shouldCapture(hasSnapshot: true, panelIsKey: true))
    }

    @Test("refreshes the snapshot after focus moves elsewhere")
    func refreshesStaleSnapshot() {
        #expect(QuickTerminalFocusRestorationPolicy.shouldCapture(hasSnapshot: true, panelIsKey: false))
    }

    @Test("restores focus only when requested and still owned by the panel", arguments: [
        (true, true, true),
        (true, false, false),
        (false, true, false),
        (false, false, false),
    ])
    func restorationDecision(requested: Bool, panelIsKey: Bool, expected: Bool) {
        #expect(QuickTerminalFocusRestorationPolicy.shouldRestore(
            requested: requested,
            panelIsKey: panelIsKey
        ) == expected)
    }
}
