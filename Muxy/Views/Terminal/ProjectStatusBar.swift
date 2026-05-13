import AppKit
import SwiftUI

struct ProjectStatusBar: View {
    let activePane: TerminalPaneState?
    let activeWorktree: Worktree?
    let isInteractive: Bool
    let richInputVisible: Bool
    @Binding var richInputFontSize: Double

    private var richInputShortcutLabel: String {
        KeyBindingStore.shared.combo(for: .toggleRichInput).displayString
    }

    private var voiceShortcutLabel: String {
        KeyBindingStore.shared.combo(for: .toggleVoiceRecording).displayString
    }

    var body: some View {
        HStack(spacing: 8) {
            if let pane = activePane {
                pathButton(pane)
                if let worktree = activeWorktree {
                    separator
                    worktreeLabel(worktree)
                }
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
                separator
                voiceRecordingButton
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

    private func pathButton(_ pane: TerminalPaneState) -> some View {
        let fullPath = pane.currentWorkingDirectory ?? pane.projectPath
        let displayPath = abbreviatePath(fullPath)
        let truncated = ProjectStatusBar.truncatePath(displayPath, maxCharacters: ProjectStatusBar.pathMaxCharacters)
        return Button {
            revealInFinder(fullPath)
        } label: {
            HStack(spacing: 4) {
                Image(systemName: "folder")
                    .font(.system(size: 10, weight: .semibold))
                Text(truncated)
                    .font(.system(size: 11, weight: .medium))
                    .lineLimit(1)
            }
            .foregroundStyle(MuxyTheme.fgMuted)
        }
        .buttonStyle(.plain)
        .help(fullPath)
        .accessibilityLabel("Reveal \(fullPath) in Finder")
        .contextMenu {
            Button("Copy Path") { copyToPasteboard(fullPath) }
            Button("Reveal in Finder") { revealInFinder(fullPath) }
        }
    }

    private func worktreeLabel(_ worktree: Worktree) -> some View {
        HStack(spacing: 4) {
            Image(systemName: "square.stack.3d.up")
                .font(.system(size: 10, weight: .semibold))
            Text(worktree.name)
                .font(.system(size: 11, weight: .medium))
                .lineLimit(1)
                .truncationMode(.tail)
        }
        .foregroundStyle(MuxyTheme.fgMuted)
        .help("Worktree: \(worktree.name)")
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

    private var voiceRecordingButton: some View {
        Button(action: handleToggleVoiceRecording) {
            HStack(spacing: 4) {
                Image(systemName: "mic")
                    .font(.system(size: 11, weight: .semibold))
                Text(voiceShortcutLabel)
                    .font(.system(size: 10, weight: .medium, design: .rounded))
                    .foregroundStyle(MuxyTheme.fgDim)
            }
        }
        .buttonStyle(RichInputToolbarButtonStyle())
        .disabled(!isInteractive)
        .accessibilityLabel("Start Voice Recording")
        .help("Start Voice Recording")
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

    private func handleToggleVoiceRecording() {
        guard isInteractive else { return }
        NotificationCenter.default.post(name: .toggleVoiceRecording, object: nil)
    }

    private func revealInFinder(_ path: String) {
        let url = URL(fileURLWithPath: path)
        NSWorkspace.shared.activateFileViewerSelecting([url])
    }

    private func copyToPasteboard(_ string: String) {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(string, forType: .string)
    }

    private func abbreviatePath(_ path: String) -> String {
        let home = NSHomeDirectory()
        guard !home.isEmpty, path.hasPrefix(home) else { return path }
        return "~" + path.dropFirst(home.count)
    }

    static let pathMaxCharacters = 40

    static func truncatePath(_ path: String, maxCharacters: Int) -> String {
        guard path.count > maxCharacters, maxCharacters > 1 else { return path }
        let suffix = path.suffix(maxCharacters - 1)
        return "…" + suffix
    }
}
