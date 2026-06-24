import AppKit
import SwiftUI
import Testing

@testable import Muxy

@MainActor
@Suite("ExtensionConsentDialog")
struct ExtensionConsentDialogTests {
    @Test("sheet height is capped to the visible screen")
    func sheetHeightIsCappedToVisibleScreen() {
        let fitting = NSSize(width: 520, height: 1_200)
        let visibleFrame = NSRect(x: 0, y: 0, width: 1_200, height: 640)

        let size = ExtensionConsentSheetLayout.contentSize(for: fitting, visibleFrame: visibleFrame)

        #expect(size.width == 520)
        #expect(size.height == 560)
    }

    @Test("long shell command keeps dialog fitting height bounded")
    func longShellCommandKeepsDialogFittingHeightBounded() {
        let command = String(repeating: "printf '%s' very-long-extension-command && ", count: 300)
        let request = ExtensionConsentRequest(
            extensionID: "demo-long-command",
            extensionDisplayName: "Long Command Demo",
            verb: .exec,
            payload: .exec(argv: nil, shell: command),
            payloadSummary: "sh -c …",
            payloadDetails: ["shell: \(command)"],
            suggestedMatch: .shellExact(command),
            source: "test"
        )
        let view = ExtensionConsentDialog(request: request, onChoice: { _ in })
        let hostingView = NSHostingView(rootView: view)
        hostingView.layoutSubtreeIfNeeded()

        #expect(hostingView.fittingSize.height <= 620)
    }

    @Test("long remember rule keeps dialog fitting height bounded")
    func longRememberRuleKeepsDialogFittingHeightBounded() {
        let command = String(repeating: "remember-rule-segment-", count: 300)
        let request = ExtensionConsentRequest(
            extensionID: "demo-long-command",
            extensionDisplayName: "Long Command Demo",
            verb: .exec,
            payload: .exec(argv: nil, shell: command),
            payloadSummary: "sh -c …",
            payloadDetails: ["shell: short"],
            suggestedMatch: .shellExact(command),
            source: "test"
        )
        let view = ExtensionConsentDialog(request: request, onChoice: { _ in })
        let hostingView = NSHostingView(rootView: view)
        hostingView.layoutSubtreeIfNeeded()

        #expect(hostingView.fittingSize.height <= 260)
    }
}
