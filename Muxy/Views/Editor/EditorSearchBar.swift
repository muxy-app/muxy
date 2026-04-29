import SwiftUI

struct EditorSearchBar: View {
    @Bindable var state: EditorTabState
    let onNext: () -> Void
    let onPrevious: () -> Void
    let onReplace: () -> Void
    let onReplaceAll: () -> Void
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
            HStack(alignment: .top, spacing: 4) {
                Button {
                    state.replaceVisible.toggle()
                } label: {
                    Image(systemName: state.replaceVisible ? "chevron.down" : "chevron.right")
                        .font(.system(size: 10, weight: .semibold))
                }
                .buttonStyle(EditorSearchButtonStyle())
                .help(state.replaceVisible ? "Hide Replace" : "Show Replace")
                .accessibilityLabel(state.replaceVisible ? "Hide Replace" : "Show Replace")
                .padding(.top, 1)

                VStack(spacing: 4) {
                    searchRow
                    if state.replaceVisible {
                        replaceRow
                    }
                }
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 6)
            .background(MuxyTheme.bg.opacity(0.95))

            Rectangle().fill(MuxyTheme.border).frame(height: 1)
        }
        .deferFocus($isFieldFocused, on: state.searchFocusVersion)
        .onKeyPress(.escape) {
            onClose()
            return .handled
        }
    }

    private var searchRow: some View {
        HStack(spacing: 6) {
            HStack(spacing: 4) {
                Image(systemName: "magnifyingglass")
                    .font(.system(size: 11))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .accessibilityHidden(true)

                TextField("Search", text: $state.searchNeedle)
                    .textFieldStyle(.plain)
                    .font(.system(size: 12))
                    .foregroundStyle(MuxyTheme.fg)
                    .focused($isFieldFocused)
                    .onSubmit { onNext() }
                    .onKeyPress(.return, phases: .down) { press in
                        guard press.modifiers.contains(.shift) else { return .ignored }
                        onPrevious()
                        return .handled
                    }

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
            .accessibilityLabel("Previous Match")

            Button(action: onNext) {
                Image(systemName: "chevron.down")
                    .font(.system(size: 10, weight: .semibold))
            }
            .buttonStyle(EditorSearchButtonStyle())
            .accessibilityLabel("Next Match")

            Button(action: onClose) {
                Image(systemName: "xmark")
                    .font(.system(size: 10, weight: .semibold))
            }
            .buttonStyle(EditorSearchButtonStyle())
            .accessibilityLabel("Close Search")
        }
    }

    private var replaceRow: some View {
        HStack(spacing: 6) {
            HStack(spacing: 4) {
                Image(systemName: "arrow.2.squarepath")
                    .font(.system(size: 11))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .accessibilityHidden(true)

                TextField("Replace", text: $state.replaceText)
                    .textFieldStyle(.plain)
                    .font(.system(size: 12))
                    .foregroundStyle(MuxyTheme.fg)
                    .onSubmit(onReplace)
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(MuxyTheme.surface)
            .clipShape(RoundedRectangle(cornerRadius: 6))
            .overlay(
                RoundedRectangle(cornerRadius: 6)
                    .strokeBorder(MuxyTheme.border, lineWidth: 1)
            )

            Button("Replace", action: onReplace)
                .buttonStyle(EditorSearchTextButtonStyle())
                .disabled(state.searchMatchCount == 0)

            Button("All", action: onReplaceAll)
                .buttonStyle(EditorSearchTextButtonStyle())
                .disabled(state.searchMatchCount == 0)
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

private struct EditorSearchTextButtonStyle: ButtonStyle {
    @Environment(\.isEnabled) private var isEnabled

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 11, weight: .medium))
            .foregroundStyle(isEnabled ? MuxyTheme.fg : MuxyTheme.fgDim)
            .padding(.horizontal, 8)
            .frame(height: 22)
            .background(configuration.isPressed ? MuxyTheme.surface : MuxyTheme.bg)
            .overlay(
                RoundedRectangle(cornerRadius: 4)
                    .strokeBorder(MuxyTheme.border, lineWidth: 1)
            )
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
        .accessibilityLabel(help)
        .accessibilityValue(isOn ? "Enabled" : "Disabled")
        .accessibilityAddTraits(.isToggle)
    }
}
