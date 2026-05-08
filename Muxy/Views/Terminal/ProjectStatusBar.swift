import AppKit
import SwiftUI

struct ProjectStatusBar: View {
    let activePane: TerminalPaneState?
    let isInteractive: Bool
    let richInputVisible: Bool
    @Binding var richInputFontSize: Double

    private var richInputShortcutLabel: String {
        KeyBindingStore.shared.combo(for: .toggleRichInput).displayString
    }

    var body: some View {
        HStack(spacing: 8) {
            if let pane = activePane {
                cwdLabel(pane)
                if let branch = pane.branchObserver.branch {
                    separator
                    branchLabel(branch)
                }
            }
            Spacer(minLength: 8)
            if richInputVisible {
                zoomControls
                separator
                shortcutHints
                separator
            }
            if activePane != nil {
                richInputToggleButton
            }
        }
        .padding(.horizontal, 10)
        .frame(height: 28)
        .background(MuxyTheme.bg)
        .overlay(
            Rectangle().fill(MuxyTheme.border).frame(height: 1),
            alignment: .top
        )
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Status bar")
    }

    private func cwdLabel(_ pane: TerminalPaneState) -> some View {
        HStack(spacing: 4) {
            Image(systemName: "folder")
                .font(.system(size: 10, weight: .semibold))
            Text(abbreviatePath(pane.currentWorkingDirectory ?? pane.projectPath))
                .font(.system(size: 11, weight: .medium))
                .lineLimit(1)
                .truncationMode(.middle)
        }
        .foregroundStyle(MuxyTheme.fgMuted)
        .help(pane.currentWorkingDirectory ?? pane.projectPath)
    }

    private func branchLabel(_ branch: String) -> some View {
        HStack(spacing: 4) {
            Image(systemName: "arrow.triangle.branch")
                .font(.system(size: 10, weight: .semibold))
            Text(branch)
                .font(.system(size: 11, weight: .medium))
                .lineLimit(1)
                .truncationMode(.middle)
        }
        .foregroundStyle(MuxyTheme.fgMuted)
        .help("Branch: \(branch)")
    }

    private var separator: some View {
        Rectangle()
            .fill(MuxyTheme.border)
            .frame(width: 1)
            .frame(maxHeight: .infinity)
            .accessibilityHidden(true)
    }

    private var richInputToggleButton: some View {
        Button(action: handleToggleRichInput) {
            HStack(spacing: 4) {
                Image(systemName: "keyboard")
                    .font(.system(size: 11, weight: .semibold))
                Text(richInputShortcutLabel)
                    .font(.system(size: 10, weight: .medium, design: .rounded))
                    .foregroundStyle(MuxyTheme.fgDim)
            }
        }
        .buttonStyle(RichInputToolbarButtonStyle())
        .disabled(!isInteractive)
        .accessibilityLabel("Toggle Rich Input")
        .help("Toggle Rich Input")
    }

    private var zoomControls: some View {
        HStack(spacing: 2) {
            Button(action: decreaseFontSize) {
                Image(systemName: "textformat.size.smaller")
                    .font(.system(size: 11, weight: .semibold))
            }
            .buttonStyle(RichInputToolbarButtonStyle())
            .disabled(richInputFontSize <= RichInputPreferences.minFontSize)
            .accessibilityLabel("Decrease editor font size")
            .help("Decrease font size")

            Text("\(Int(clampedFontSize))")
                .font(.system(size: 10, weight: .medium, design: .rounded))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(minWidth: 18)
                .accessibilityLabel("Editor font size \(Int(clampedFontSize))")

            Button(action: increaseFontSize) {
                Image(systemName: "textformat.size.larger")
                    .font(.system(size: 11, weight: .semibold))
            }
            .buttonStyle(RichInputToolbarButtonStyle())
            .disabled(richInputFontSize >= RichInputPreferences.maxFontSize)
            .accessibilityLabel("Increase editor font size")
            .help("Increase font size")
        }
    }

    private var shortcutHints: some View {
        let store = KeyBindingStore.shared
        let submit = store.combo(for: .submitRichInput).displayString
        let submitNoReturn = store.combo(for: .submitRichInputWithoutReturn).displayString
        return HStack(spacing: 10) {
            shortcutHint(keys: submit, label: "Send")
            shortcutHint(keys: submitNoReturn, label: "Send w/o ↩")
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(submit) Send. \(submitNoReturn) Send without Enter.")
    }

    private func shortcutHint(keys: String, label: String) -> some View {
        HStack(spacing: 4) {
            Text(keys)
                .font(.system(size: 10, weight: .medium, design: .rounded))
                .foregroundStyle(MuxyTheme.fgMuted)
            Text(label)
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(MuxyTheme.fgMuted)
        }
    }

    private var clampedFontSize: Double {
        min(max(richInputFontSize, RichInputPreferences.minFontSize), RichInputPreferences.maxFontSize)
    }

    private func decreaseFontSize() {
        richInputFontSize = max(RichInputPreferences.minFontSize, richInputFontSize - RichInputPreferences.fontStep)
    }

    private func increaseFontSize() {
        richInputFontSize = min(RichInputPreferences.maxFontSize, richInputFontSize + RichInputPreferences.fontStep)
    }

    private func handleToggleRichInput() {
        guard isInteractive else { return }
        NotificationCenter.default.post(name: .toggleRichInput, object: nil)
    }

    private func abbreviatePath(_ path: String) -> String {
        let home = NSHomeDirectory()
        guard !home.isEmpty, path.hasPrefix(home) else { return path }
        return "~" + path.dropFirst(home.count)
    }
}
