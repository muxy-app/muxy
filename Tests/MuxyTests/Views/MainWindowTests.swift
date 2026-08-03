import Foundation
import Testing
@testable import Muxy

@Suite("OverlayEscapeDecision")
struct OverlayEscapeDecisionTests {
    @Test func consumesEscapeWhenOverlayActive() {
        #expect(OverlayEscapeDecision.shouldConsume(isOverlayActive: true, keyCode: 53))
    }

    @Test func passesEscapeThroughWhenOverlayInactive() {
        #expect(!OverlayEscapeDecision.shouldConsume(isOverlayActive: false, keyCode: 53))
    }

    @Test func passesNonEscapeThroughWhenOverlayActive() {
        #expect(!OverlayEscapeDecision.shouldConsume(isOverlayActive: true, keyCode: 36))
    }

    @Test func passesNonEscapeThroughWhenOverlayInactive() {
        #expect(!OverlayEscapeDecision.shouldConsume(isOverlayActive: false, keyCode: 36))
    }
}

@Suite("Main window modal policy")
struct MainWindowModalPolicyTests {
    @Test func permitsAnInactiveOverlay() {
        #expect(MainWindowModalPolicy.canPresent(.composer, active: []))
    }

    @Test func permitsTheAlreadyActiveOverlay() {
        #expect(MainWindowModalPolicy.canPresent(.composer, active: [.composer]))
    }

    @Test func rejectsPresentationOverAnotherOverlay() {
        #expect(!MainWindowModalPolicy.canPresent(.composer, active: [.terminalOmnibox]))
        #expect(!MainWindowModalPolicy.canPresent(.projectPicker, active: [.composer]))
    }

    @Test func resolvesTheVisuallyTopmostOverlay() {
        #expect(MainWindowModalPolicy.topmost(in: [.composer, .terminalOmnibox]) == .terminalOmnibox)
        #expect(MainWindowModalPolicy.topmost(in: [.composer, .extensionModal]) == .extensionModal)
        #expect(
            MainWindowModalPolicy.topmost(in: [.extensionModal, .extensionWebviewModal])
                == .extensionWebviewModal
        )
    }
}

@Suite("Main window voice input policy")
struct MainWindowVoiceInputPolicyTests {
    @Test func opensLegacyPanelWithoutComposer() {
        #expect(MainWindowVoiceInputPolicy.target(composerVisible: false) == .legacyPanel)
    }

    @Test func activatesVoiceInVisibleComposer() {
        #expect(MainWindowVoiceInputPolicy.target(composerVisible: true) == .composer)
    }
}

@Suite("Pane close control policy")
@MainActor
struct PaneCloseControlPolicyTests {
    @Test("shows the close control for an unpinned split target")
    func splitTargetVisibility() {
        let area = TabArea(projectPath: "/tmp/test")
        let target = PaneCloseControlPolicy.target(
            isActiveGroup: true,
            focusedAreaID: area.id,
            panes: [(area, area.activeTab!)]
        )

        #expect(PaneCloseControlPolicy.isVisible(paneCount: 2, target: target))
    }

    @Test("hides the close control without a split, target, or closable tab")
    func unavailableTargetVisibility() {
        let area = TabArea(projectPath: "/tmp/test")
        let tab = area.activeTab!
        let target = PaneCloseTarget(areaID: area.id, tabID: tab.id, isPinned: false)
        let pinnedTarget = PaneCloseTarget(areaID: area.id, tabID: tab.id, isPinned: true)

        #expect(!PaneCloseControlPolicy.isVisible(paneCount: 1, target: target))
        #expect(!PaneCloseControlPolicy.isVisible(paneCount: 2, target: nil))
        #expect(!PaneCloseControlPolicy.isVisible(paneCount: 2, target: pinnedTarget))
    }

    @Test("does not target an inactive group or an area outside the visible layout")
    func explicitTargeting() {
        let area = TabArea(projectPath: "/tmp/test")
        let panes = [(area: area, tab: area.activeTab!)]

        #expect(PaneCloseControlPolicy.target(
            isActiveGroup: false,
            focusedAreaID: area.id,
            panes: panes
        ) == nil)
        #expect(PaneCloseControlPolicy.target(
            isActiveGroup: true,
            focusedAreaID: UUID(),
            panes: panes
        ) == nil)
    }
}
