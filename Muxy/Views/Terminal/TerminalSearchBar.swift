import SwiftUI

struct TerminalSearchBar: View {
    @Bindable var searchState: TerminalSearchState
    let onNavigateNext: () -> Void
    let onNavigatePrevious: () -> Void
    let onClose: () -> Void

    @FocusState private var isFieldFocused: Bool

    var body: some View {
        HStack(spacing: 6) {
            HStack(spacing: 4) {
                Image(systemName: "magnifyingglass")
                    .font(.system(size: 11))
                    .foregroundStyle(MuxyTheme.fgMuted)

                TextField("Search", text: $searchState.needle)
                    .textFieldStyle(.plain)
                    .font(.system(size: 12))
                    .foregroundStyle(MuxyTheme.fg)
                    .focused($isFieldFocused)
                    .onSubmit { onNavigateNext() }
                    .onChange(of: searchState.needle) {
                        searchState.pushNeedle()
                    }

                if !searchState.displayText.isEmpty {
                    Text(searchState.displayText)
                        .font(.system(size: 10))
                        .foregroundStyle(MuxyTheme.fgMuted)
                        .lineLimit(1)
                        .fixedSize()
                }
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 5)
            .background(MuxyTheme.surface)
            .clipShape(RoundedRectangle(cornerRadius: 6))
            .overlay(
                RoundedRectangle(cornerRadius: 6)
                    .strokeBorder(MuxyTheme.border, lineWidth: 1)
            )

            Button(action: onNavigatePrevious) {
                Image(systemName: "chevron.up")
                    .font(.system(size: 10, weight: .semibold))
            }
            .buttonStyle(SearchBarButtonStyle())

            Button(action: onNavigateNext) {
                Image(systemName: "chevron.down")
                    .font(.system(size: 10, weight: .semibold))
            }
            .buttonStyle(SearchBarButtonStyle())

            Button(action: onClose) {
                Image(systemName: "xmark")
                    .font(.system(size: 10, weight: .semibold))
            }
            .buttonStyle(SearchBarButtonStyle())
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 6)
        .background(MuxyTheme.bg.opacity(0.95))
        .onAppear { isFieldFocused = true }
        .onKeyPress(.escape) {
            onClose()
            return .handled
        }
    }
}

private struct SearchBarButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .frame(width: 22, height: 22)
            .contentShape(Rectangle())
            .foregroundStyle(MuxyTheme.fgMuted)
            .background(configuration.isPressed ? MuxyTheme.surface : .clear)
            .clipShape(RoundedRectangle(cornerRadius: 4))
    }
}
