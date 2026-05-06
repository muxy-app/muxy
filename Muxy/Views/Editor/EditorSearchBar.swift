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
            HStack(alignment: .top, spacing: UIMetrics.spacing2) {
                Button {
                    state.replaceVisible.toggle()
                } label: {
                    Image(systemName: state.replaceVisible ? "chevron.down" : "chevron.right")
                        .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                }
                .buttonStyle(EditorSearchButtonStyle())
                .help(state.replaceVisible ? "Hide Replace" : "Show Replace")
                .accessibilityLabel(state.replaceVisible ? "Hide Replace" : "Show Replace")
                .padding(.top, UIMetrics.scaled(1))

                VStack(spacing: UIMetrics.spacing2) {
                    searchRow
                    if state.replaceVisible {
                        replaceRow
                    }
                }
            }
            .padding(.horizontal, UIMetrics.spacing4)
            .padding(.vertical, UIMetrics.spacing3)
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
        HStack(spacing: UIMetrics.spacing3) {
            HStack(spacing: UIMetrics.spacing2) {
                Image(systemName: "magnifyingglass")
                    .font(.system(size: UIMetrics.fontFootnote))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .accessibilityHidden(true)

                TextField("Search", text: $state.searchNeedle)
                    .textFieldStyle(.plain)
                    .font(.system(size: UIMetrics.fontBody))
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
                        .font(.system(size: UIMetrics.fontCaption))
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
            .padding(.horizontal, UIMetrics.spacing4)
            .padding(.vertical, UIMetrics.spacing2)
            .background(MuxyTheme.surface)
            .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
            .overlay(
                RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
                    .strokeBorder(MuxyTheme.border, lineWidth: 1)
            )

            Button(action: onPrevious) {
                Image(systemName: "chevron.up")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
            }
            .buttonStyle(EditorSearchButtonStyle())
            .accessibilityLabel("Previous Match")

            Button(action: onNext) {
                Image(systemName: "chevron.down")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
            }
            .buttonStyle(EditorSearchButtonStyle())
            .accessibilityLabel("Next Match")

            Button(action: onClose) {
                Image(systemName: "xmark")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
            }
            .buttonStyle(EditorSearchButtonStyle())
            .accessibilityLabel("Close Search")
        }
    }

    private var replaceRow: some View {
        HStack(spacing: UIMetrics.spacing3) {
            HStack(spacing: UIMetrics.spacing2) {
                Image(systemName: "arrow.2.squarepath")
                    .font(.system(size: UIMetrics.fontFootnote))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .accessibilityHidden(true)

                TextField("Replace", text: $state.replaceText)
                    .textFieldStyle(.plain)
                    .font(.system(size: UIMetrics.fontBody))
                    .foregroundStyle(MuxyTheme.fg)
                    .onSubmit(onReplace)
            }
            .padding(.horizontal, UIMetrics.spacing4)
            .padding(.vertical, UIMetrics.spacing2)
            .background(MuxyTheme.surface)
            .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
            .overlay(
                RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
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
            .frame(width: UIMetrics.scaled(22), height: UIMetrics.scaled(22))
            .contentShape(Rectangle())
            .foregroundStyle(MuxyTheme.fgMuted)
            .background(configuration.isPressed ? MuxyTheme.surface : .clear)
            .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
    }
}

private struct EditorSearchTextButtonStyle: ButtonStyle {
    @Environment(\.isEnabled) private var isEnabled

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
            .foregroundStyle(isEnabled ? MuxyTheme.fg : MuxyTheme.fgDim)
            .padding(.horizontal, UIMetrics.spacing4)
            .frame(height: UIMetrics.scaled(22))
            .background(configuration.isPressed ? MuxyTheme.surface : MuxyTheme.bg)
            .overlay(
                RoundedRectangle(cornerRadius: UIMetrics.radiusSM)
                    .strokeBorder(MuxyTheme.border, lineWidth: 1)
            )
            .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
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
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold, design: .monospaced))
                .foregroundStyle(isOn ? MuxyTheme.fg : MuxyTheme.fgMuted)
                .frame(width: UIMetrics.controlSmall, height: UIMetrics.scaled(18))
                .background(isOn ? MuxyTheme.border : .clear)
                .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
        }
        .buttonStyle(.plain)
        .help(help)
        .accessibilityLabel(help)
        .accessibilityValue(isOn ? "Enabled" : "Disabled")
        .accessibilityAddTraits(.isToggle)
    }
}
