import SwiftUI

struct TerminalSearchBar: View {
    @Bindable var searchState: TerminalSearchState
    let onNavigateNext: () -> Void
    let onNavigatePrevious: () -> Void
    let onClose: () -> Void

    @FocusState private var isFieldFocused: Bool

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: UIMetrics.spacing3) {
                HStack(spacing: UIMetrics.spacing2) {
                    Image(systemName: "magnifyingglass")
                        .font(.system(size: UIMetrics.fontFootnote))
                        .foregroundStyle(MuxyTheme.fgMuted)
                        .accessibilityHidden(true)

                    TextField("Search", text: $searchState.needle)
                        .textFieldStyle(.plain)
                        .font(.system(size: UIMetrics.fontBody))
                        .foregroundStyle(MuxyTheme.fg)
                        .focused($isFieldFocused)
                        .onSubmit { onNavigateNext() }
                        .onChange(of: searchState.needle) {
                            searchState.pushNeedle()
                        }

                    if !searchState.displayText.isEmpty {
                        Text(searchState.displayText)
                            .font(.system(size: UIMetrics.fontCaption))
                            .foregroundStyle(MuxyTheme.fgMuted)
                            .lineLimit(1)
                            .fixedSize()
                            .accessibilityLabel("Search results: \(searchState.displayText)")
                    }
                }
                .padding(.horizontal, UIMetrics.spacing4)
                .padding(.vertical, UIMetrics.spacing2)
                .background(MuxyTheme.surface)
                .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
                .overlay(
                    RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
                        .strokeBorder(MuxyTheme.border, lineWidth: 1)
                )

                Button(action: onNavigatePrevious) {
                    Image(systemName: "chevron.up")
                        .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                }
                .buttonStyle(SearchBarButtonStyle())
                .accessibilityLabel("Previous Match")

                Button(action: onNavigateNext) {
                    Image(systemName: "chevron.down")
                        .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                }
                .buttonStyle(SearchBarButtonStyle())
                .accessibilityLabel("Next Match")

                Button(action: onClose) {
                    Image(systemName: "xmark")
                        .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                }
                .buttonStyle(SearchBarButtonStyle())
                .accessibilityLabel("Close Search")
            }
            .padding(.horizontal, UIMetrics.spacing4)
            .frame(height: UIMetrics.scaled(32))
            .background(MuxyTheme.bg.opacity(0.95))

            Rectangle().fill(MuxyTheme.border).frame(height: 1)
        }
        .deferFocus($isFieldFocused, on: searchState.focusVersion)
        .onKeyPress(.escape) {
            onClose()
            return .handled
        }
    }
}

private struct SearchBarButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .frame(width: UIMetrics.scaled(22), height: UIMetrics.scaled(22))
            .contentShape(Rectangle())
            .foregroundStyle(MuxyTheme.fgMuted)
            .background(configuration.isPressed ? MuxyTheme.surface : .clear)
            .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
    }
}
