import SwiftUI

struct QuickTerminalView: View {
    let paneState: TerminalPaneState

    var body: some View {
        TerminalPane(
            state: paneState,
            focused: true,
            onFocus: {},
            onProcessExit: {},
            onSplitRequest: { _, _ in }
        )
        .background(Color(nsColor: GhosttyService.shared.backgroundColor))
    }
}
