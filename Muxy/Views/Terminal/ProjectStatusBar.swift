import AppKit
import SwiftUI

struct ProjectStatusBar: View {
    struct StatusContext: Equatable {
        let path: String
    }

    let activePane: TerminalPaneState?
    let activeWorktree: Worktree?
    let fallbackProjectPath: String?
    let isInteractive: Bool
    let richInputVisible: Bool
    @Binding var richInputFontSize: Double
    @Binding var extensionOutputVisible: Bool
    var onTriggerExtensionCommand: ((ExtensionStore.StatusBarItemBinding) -> Void)?
    @Environment(ExtensionStore.self) private var extensionStore
    @State private var popoverHost = PopoverHost.shared

    private var richInputShortcutLabel: String {
        KeyBindingStore.shared.combo(for: .toggleRichInput).displayString
    }

    private var voiceShortcutLabel: String {
        KeyBindingStore.shared.combo(for: .toggleVoiceRecording).displayString
    }

    var body: some View {
        HStack(spacing: 8) {
            leftSide
            Spacer(minLength: 8)
            rightSide
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

    private var leftSide: some View {
        HStack(spacing: 8) {
            if let statusContext {
                pathButton(statusContext.path)
                separator
            }
            ForEach(extensionStore.statusBarItems(side: .left)) { binding in
                extensionItem(binding: binding)
                separator
            }
        }
    }

    private var rightSide: some View {
        HStack(spacing: 8) {
            separator
            extensionOutputChip
            ForEach(extensionStore.statusBarItems(side: .right)) { binding in
                separator
                extensionItem(binding: binding)
            }
            if richInputVisible {
                separator
                zoomControls
                separator
                shortcutHints
            }
            if activePane != nil {
                separator
                richInputToggleButton
                separator
                voiceRecordingButton
            }
        }
    }

    private var statusContext: StatusContext? {
        Self.statusContext(
            activePane: activePane,
            activeWorktree: activeWorktree,
            fallbackProjectPath: fallbackProjectPath
        )
    }

    static func statusContext(
        activePane: TerminalPaneState?,
        activeWorktree: Worktree?,
        fallbackProjectPath: String?
    ) -> StatusContext? {
        guard let path = activePane?.currentWorkingDirectory
            ?? activePane?.projectPath
            ?? activeWorktree?.path
            ?? fallbackProjectPath
        else { return nil }
        return StatusContext(path: path)
    }

    private func pathButton(_ fullPath: String) -> some View {
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

    private var separator: some View {
        Rectangle()
            .fill(MuxyTheme.border)
            .frame(width: 1)
            .frame(maxHeight: .infinity)
            .accessibilityHidden(true)
    }

    private func extensionItem(binding: ExtensionStore.StatusBarItemBinding) -> some View {
        let popover = extensionStore.popover(for: binding.muxyExtension, command: binding.item.command)
        return Button {
            if let popover {
                popoverHost.toggle(
                    anchorID: binding.id,
                    extensionID: binding.muxyExtension.id,
                    popover: popover,
                    data: nil
                )
            } else {
                onTriggerExtensionCommand?(binding)
            }
        } label: {
            HStack(spacing: 4) {
                ExtensionIconView(
                    icon: binding.displayIcon,
                    muxyExtension: binding.muxyExtension,
                    size: 10
                )
                if let text = binding.displayText, !text.isEmpty {
                    Text(text)
                        .font(.system(size: 11, weight: .medium))
                        .lineLimit(1)
                }
            }
            .foregroundStyle(MuxyTheme.fgMuted)
            .padding(.horizontal, 4)
            .frame(maxHeight: .infinity)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .padding(.horizontal, -4)
        .help(binding.item.tooltip ?? binding.item.id)
        .accessibilityLabel(binding.item.tooltip ?? binding.item.id)
        .extensionPopover(anchorID: binding.id, host: popoverHost)
    }

    private var extensionOutputChip: some View {
        Button {
            extensionOutputVisible.toggle()
        } label: {
            Image(systemName: "ladybug")
                .font(.system(size: 10, weight: .semibold))
                .foregroundStyle(extensionOutputVisible ? MuxyTheme.accent : MuxyTheme.fgMuted)
        }
        .buttonStyle(.plain)
        .help("Toggle Extension Output panel")
        .accessibilityLabel("Toggle Extension Output")
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
