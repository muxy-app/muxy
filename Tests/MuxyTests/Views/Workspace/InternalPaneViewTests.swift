import Foundation
import Testing

@testable import Muxy

@Suite("InternalPaneView")
struct InternalPaneViewTests {
    @Test("child focus requires the parent area to be focused")
    func childFocusRequiresParentFocus() {
        let paneID = UUID()

        #expect(InternalPanePresentation.isFocused(
            paneID: paneID,
            focusedPaneID: paneID,
            parentFocused: false
        ) == false)
        #expect(InternalPanePresentation.isFocused(
            paneID: paneID,
            focusedPaneID: paneID,
            parentFocused: true
        ))
    }

    @Test("child visibility follows the parent area")
    func childVisibilityFollowsParentVisibility() {
        #expect(InternalPanePresentation.isVisible(parentVisible: false) == false)
        #expect(InternalPanePresentation.isVisible(parentVisible: true))
    }

    @Test("process exit is routed to the exiting child pane")
    func processExitTargetsChildPane() {
        let paneID = UUID()
        var exitedPaneID: UUID?
        let handler = InternalPanePresentation.processExitHandler(paneID: paneID) {
            exitedPaneID = $0
        }

        handler()

        #expect(exitedPaneID == paneID)
    }

    @Test("the final internal pane exit closes its tab")
    func finalInternalPaneExitClosesTab() {
        let paneID = UUID()

        #expect(InternalPaneProcessExitAction.resolve(
            paneID: paneID,
            internalPaneCount: 1
        ) == .closeTab)
        #expect(InternalPaneProcessExitAction.resolve(
            paneID: paneID,
            internalPaneCount: 2
        ) == .closePane(paneID))
    }
}
