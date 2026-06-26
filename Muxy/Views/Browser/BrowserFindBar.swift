import SwiftUI

struct BrowserFindBar: View {
    let state: BrowserTabState
    @Binding var query: String
    @Binding var fieldFocused: Bool
    let onClose: () -> Void

    @FocusState private var focused: Bool

    var body: some View {
        HStack(spacing: UIMetrics.spacing3) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                .foregroundStyle(MuxyTheme.fgMuted)

            TextField("Find on page", text: $query)
                .textFieldStyle(.plain)
                .font(.system(size: UIMetrics.fontBody))
                .foregroundStyle(noMatch ? MuxyTheme.warning : MuxyTheme.fg)
                .focused($focused)
                .onSubmit { find(backwards: false) }
                .onExitCommand(perform: onClose)

            IconButton(symbol: "chevron.up", accessibilityLabel: "Previous Match") {
                find(backwards: true)
            }
            .disabled(query.isEmpty)

            IconButton(symbol: "chevron.down", accessibilityLabel: "Next Match") {
                find(backwards: false)
            }
            .disabled(query.isEmpty)

            IconButton(symbol: "xmark", accessibilityLabel: "Close Find", action: onClose)
        }
        .padding(.horizontal, UIMetrics.spacing4)
        .frame(height: UIMetrics.titleBarHeight)
        .background(MuxyTheme.surface)
        .overlay(alignment: .bottom) {
            Rectangle()
                .fill(MuxyTheme.border)
                .frame(height: 1)
        }
        .onChange(of: focused) { _, newValue in fieldFocused = newValue }
        .onChange(of: query) { _, _ in state.findFoundMatch = true }
        .onAppear { focused = true }
    }

    private var noMatch: Bool {
        !query.isEmpty && !state.findFoundMatch
    }

    private func find(backwards: Bool) {
        guard !query.isEmpty else { return }
        state.pendingFind = BrowserTabState.FindRequest(query: query, backwards: backwards)
    }
}
