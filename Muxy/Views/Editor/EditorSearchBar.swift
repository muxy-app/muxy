import SwiftUI

struct EditorSearchBar: View {
    @Bindable var state: EditorTabState
    let onNext: () -> Void
    let onPrevious: () -> Void
    let onClose: () -> Void

    @FocusState private var isFieldFocused: Bool

    private var displayText: String {
        guard !state.searchNeedle.isEmpty else { return "" }
        if state.searchUseRegex, state.searchInvalidRegex { return "Invalid regex" }
        guard state.searchMatchCount > 0 else { return "No results" }
        return "\(state.searchCurrentIndex) of \(state.searchMatchCount)"
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 6) {
                HStack(spacing: 4) {
                    Image(systemName: "magnifyingglass")
                        .font(.system(size: 11))
                        .foregroundStyle(MuxyTheme.fgMuted)

                    TextField("Search", text: $state.searchNeedle)
                        .textFieldStyle(.plain)
                        .font(.system(size: 12))
                        .foregroundStyle(MuxyTheme.fg)
                        .focused($isFieldFocused)
                        .onSubmit { onNext() }

                    if !displayText.isEmpty {
                        Text(displayText)
                            .font(.system(size: 10))
                            .foregroundStyle(MuxyTheme.fgMuted)
                            .lineLimit(1)
                            .fixedSize()
                    }

                    EditorSearchOptionToggle(
                        label: "Aa",
                        isOn: $state.searchCaseSensitive,
                        help: "Match Case"
                    )

                    EditorSearchOptionToggle(
                        label: ".*",
                        isOn: $state.searchUseRegex,
                        help: "Regular Expression"
                    )
                }
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
                .background(MuxyTheme.surface)
                .clipShape(RoundedRectangle(cornerRadius: 6))
                .overlay(
                    RoundedRectangle(cornerRadius: 6)
                        .strokeBorder(MuxyTheme.border, lineWidth: 1)
                )

                Button(action: onPrevious) {
                    Image(systemName: "chevron.up")
                        .font(.system(size: 10, weight: .semibold))
                }
                .buttonStyle(EditorSearchButtonStyle())

                Button(action: onNext) {
                    Image(systemName: "chevron.down")
                        .font(.system(size: 10, weight: .semibold))
                }
                .buttonStyle(EditorSearchButtonStyle())

                Button(action: onClose) {
                    Image(systemName: "xmark")
                        .font(.system(size: 10, weight: .semibold))
                }
                .buttonStyle(EditorSearchButtonStyle())
            }
            .padding(.horizontal, 8)
            .frame(height: 32)
            .background(MuxyTheme.bg.opacity(0.95))

            Rectangle().fill(MuxyTheme.border).frame(height: 1)
        }
        .onAppear { isFieldFocused = true }
        .onKeyPress(.escape) {
            onClose()
            return .handled
        }
    }
}

private struct EditorSearchButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .frame(width: 22, height: 22)
            .contentShape(Rectangle())
            .foregroundStyle(MuxyTheme.fgMuted)
            .background(configuration.isPressed ? MuxyTheme.surface : .clear)
            .clipShape(RoundedRectangle(cornerRadius: 4))
    }
}

private struct EditorSearchOptionToggle: View {
    let label: String
    @Binding var isOn: Bool
    let help: String

    var body: some View {
        Button {
            isOn.toggle()
        } label: {
            Text(label)
                .font(.system(size: 10, weight: .semibold, design: .monospaced))
                .foregroundStyle(isOn ? MuxyTheme.fg : MuxyTheme.fgMuted)
                .frame(width: 20, height: 18)
                .background(isOn ? MuxyTheme.border : .clear)
                .clipShape(RoundedRectangle(cornerRadius: 4))
        }
        .buttonStyle(.plain)
        .help(help)
    }
}
