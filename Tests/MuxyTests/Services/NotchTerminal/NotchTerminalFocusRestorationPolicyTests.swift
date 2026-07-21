import Testing

@testable import Muxy

@Suite("Notch terminal focus restoration policy")
struct NotchTerminalFocusRestorationPolicyTests {
    @Test("captures an initial focus snapshot")
    func capturesInitialSnapshot() {
        #expect(NotchTerminalFocusRestorationPolicy.shouldCapture(hasSnapshot: false, panelIsKey: false))
    }

    @Test("keeps the snapshot while the panel owns focus")
    func keepsSnapshotWhilePanelIsKey() {
        #expect(!NotchTerminalFocusRestorationPolicy.shouldCapture(hasSnapshot: true, panelIsKey: true))
    }

    @Test("refreshes the snapshot after focus moves elsewhere")
    func refreshesStaleSnapshot() {
        #expect(NotchTerminalFocusRestorationPolicy.shouldCapture(hasSnapshot: true, panelIsKey: false))
    }

    @Test("restores focus only when requested and still owned by the panel", arguments: [
        (true, true, true),
        (true, false, false),
        (false, true, false),
        (false, false, false),
    ])
    func restorationDecision(requested: Bool, panelIsKey: Bool, expected: Bool) {
        #expect(NotchTerminalFocusRestorationPolicy.shouldRestore(
            requested: requested,
            panelIsKey: panelIsKey
        ) == expected)
    }
}
