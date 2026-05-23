import SwiftUI

struct BrowserFindBar: View {
    let session: BrowserSession
    @Bindable var findBar: FindBarState
    @FocusState private var queryFocused: Bool

    init(session: BrowserSession) {
        self.session = session
        findBar = session.nav.findBar
    }

    var body: some View {
        HStack(spacing: UIMetrics.spacing2) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
            TextField("Find in page", text: $findBar.query)
                .textFieldStyle(.plain)
                .font(.system(size: UIMetrics.fontBody))
                .foregroundStyle(MuxyTheme.fg)
                .focused($queryFocused)
                .onSubmit { session.performFind(forward: true) }
                .frame(minWidth: UIMetrics.scaled(180))
            resultIndicator
            IconButton(symbol: "chevron.up", accessibilityLabel: "Previous match") {
                session.performFind(forward: false)
            }
            .disabled(findBar.query.isEmpty)
            IconButton(symbol: "chevron.down", accessibilityLabel: "Next match") {
                session.performFind(forward: true)
            }
            .disabled(findBar.query.isEmpty)
            IconButton(symbol: "xmark", accessibilityLabel: "Close find bar") {
                session.dismissFindBar()
            }
        }
        .padding(.horizontal, UIMetrics.spacing4)
        .padding(.vertical, UIMetrics.spacing2)
        .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
        .overlay(
            RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
                .strokeBorder(MuxyTheme.border, lineWidth: 1)
        )
        .shadow(color: .black.opacity(0.18), radius: 8, x: 0, y: 4)
        .frame(maxWidth: UIMetrics.scaled(420))
        .onAppear { queryFocused = true }
        .onChange(of: findBar.focusVersion) { _, _ in queryFocused = true }
        .onChange(of: findBar.query) { _, _ in findBar.lastResultFound = nil }
        .onExitCommand { session.dismissFindBar() }
    }

    @ViewBuilder
    private var resultIndicator: some View {
        if let found = findBar.lastResultFound, !findBar.query.isEmpty {
            Image(systemName: found ? "checkmark.circle.fill" : "exclamationmark.circle")
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                .foregroundStyle(found ? MuxyTheme.accent : MuxyTheme.warning)
                .help(found ? "Match found" : "No matches")
        }
    }
}
