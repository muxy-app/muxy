import AppKit
import SwiftUI
import Testing

@testable import Muxy

@MainActor
@Suite("ExtensionConsentDialog")
struct ExtensionConsentDialogTests {
    init() {
        UIScale.shared.preset = .regular
    }

    @Test("sheet height is capped to the visible screen")
    func sheetHeightIsCappedToVisibleScreen() {
        let fitting = NSSize(width: 520, height: 1_200)
        let visibleFrame = NSRect(x: 0, y: 0, width: 1_200, height: 640)

        let size = ExtensionConsentSheetLayout.contentSize(for: fitting, visibleFrame: visibleFrame)

        #expect(size.width == 520)
        #expect(size.height == 560)
    }

    @Test("payload max height shrinks on short screens to keep buttons reachable")
    func payloadMaxHeightShrinksOnShortScreens() {
        let tall = NSRect(x: 0, y: 0, width: 1_200, height: 1_080)
        let short = NSRect(x: 0, y: 0, width: 1_200, height: 480)

        let fallback = ExtensionConsentSheetLayout.payloadFallbackHeight
        #expect(ExtensionConsentSheetLayout.payloadMaxHeight(for: tall) == fallback)
        #expect(ExtensionConsentSheetLayout.payloadMaxHeight(for: short) < fallback)
        #expect(ExtensionConsentSheetLayout.payloadMaxHeight(for: short) >= ExtensionConsentSheetLayout.payloadMinimumHeight)
    }

    @Test("long shell command keeps dialog fitting height bounded")
    func longShellCommandKeepsDialogFittingHeightBounded() {
        let command = String(repeating: "printf '%s' very-long-extension-command && ", count: 300)
        let request = makeRequest(payloadDetails: ["shell: \(command)"], match: .shellExact(command))
        let view = ExtensionConsentDialog(request: request, payloadMaxHeight: 260, onChoice: { _ in })
        let hostingView = NSHostingView(rootView: view)
        hostingView.layoutSubtreeIfNeeded()

        #expect(hostingView.fittingSize.height <= 620)
    }

    @Test("long remember rule keeps dialog fitting height bounded")
    func longRememberRuleKeepsDialogFittingHeightBounded() {
        let command = String(repeating: "remember-rule-segment-", count: 300)
        let request = makeRequest(payloadDetails: ["shell: short"], match: .shellExact(command))
        let view = ExtensionConsentDialog(request: request, payloadMaxHeight: 260, onChoice: { _ in })
        let hostingView = NSHostingView(rootView: view)
        hostingView.layoutSubtreeIfNeeded()

        #expect(hostingView.fittingSize.height <= 260)
    }

    private func makeRequest(payloadDetails: [String], match: ExtensionGrantMatch) -> ExtensionConsentRequest {
        ExtensionConsentRequest(
            extensionID: "demo-long-command",
            extensionDisplayName: "Long Command Demo",
            verb: .exec,
            payload: .exec(argv: nil, shell: payloadDetails.first),
            payloadSummary: "sh -c …",
            payloadDetails: payloadDetails,
            suggestedMatch: match,
            source: "test"
        )
    }
}
