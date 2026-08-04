import AppKit
import SwiftUI

struct ProjectStatusBar: View {
    struct StatusContext: Equatable {
        let path: String
    }

    let activePane: TerminalPaneState?
    let activeWorktree: Worktree?
    let fallbackProjectPath: String?
    let isRemoteWorkspace: Bool
    let isInteractive: Bool
    @Binding var extensionOutputVisible: Bool
    var onTriggerExtensionCommand: ((ExtensionStore.StatusBarItemBinding) -> Void)?
    @Environment(ExtensionStore.self) private var extensionStore
    @State private var popoverHost = PopoverHost.shared
    @AppStorage(ResourceUsagePreferences.visibleKey) private var showResourceUsage = ResourceUsagePreferences.defaultVisible
    @AppStorage(TerminalPersistentSessionPreferences.enabledKey)
    private var backgroundSessionsEnabled = TerminalPersistentSessionPreferences.defaultIsEnabled

    var body: some View {
        HStack(spacing: 8) {
            ScrollView(.horizontal, showsIndicators: false) {
                leftSide
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            rightSide
                .fixedSize(horizontal: true, vertical: false)
        }
        .padding(.horizontal, 10)
        .frame(height: UIMetrics.statusBarHeight)
        .background(AppTransparencyBackground())
        .overlay(
            Rectangle().fill(MuxyTheme.border).frame(height: 1),
            alignment: .top
        )
        .accessibilityElement(children: .contain)
        .accessibilityLabel(L10n.string("Status bar"))
    }

    private var leftSide: some View {
        HStack(spacing: 8) {
            if let statusContext {
                pathButton(statusContext.path)
                separator
            }
            RepositoryStatusBarItems()
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
            if activePane != nil {
                separator
                voiceRecordingButton
            }
            if backgroundSessionsEnabled, !isRemoteWorkspace {
                TerminalSessionsButton()
            }
            if showResourceUsage {
                separator
                ResourceUsageButton()
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
        let remote = isRemoteWorkspace
        return Button {
            if remote {
                PathClipboard.copy(fullPath)
            } else {
                revealInFinder(fullPath)
            }
        } label: {
            HStack(spacing: 4) {
                Image(systemName: remote ? "network" : "folder")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                Text(truncated)
                    .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                    .lineLimit(1)
            }
            .foregroundStyle(MuxyTheme.fgMuted)
        }
        .buttonStyle(.plain)
        .help(fullPath)
        .accessibilityLabel(
            remote
                ? L10n.string("Copy \(fullPath)")
                : L10n.string("Reveal \(fullPath) in Finder")
        )
        .contextMenu {
            Button(L10n.string("Copy Path")) { PathClipboard.copy(fullPath) }
            if !remote {
                Button(L10n.string("Reveal in Finder")) { revealInFinder(fullPath) }
            }
        }
    }

    private var separator: some View {
        StatusBarSeparator()
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
                    size: UIMetrics.iconXS
                )
                if let text = binding.displayText, !text.isEmpty {
                    Text(text)
                        .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
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
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                .foregroundStyle(extensionOutputVisible ? MuxyTheme.accent : MuxyTheme.fgMuted)
        }
        .buttonStyle(.plain)
        .help(L10n.string("Toggle Extension Output panel"))
        .accessibilityLabel(L10n.string("Toggle Extension Output"))
    }

    private var voiceRecordingButton: some View {
        Button(action: handleToggleVoiceRecording) {
            HStack(spacing: 4) {
                Image(systemName: "mic")
                    .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                if legacyVoiceShortcut.isAssigned {
                    Text(legacyVoiceShortcut.displayString)
                        .font(.system(size: UIMetrics.fontCaption, weight: .medium, design: .rounded))
                        .foregroundStyle(MuxyTheme.fgDim)
                }
            }
        }
        .buttonStyle(RichInputToolbarButtonStyle())
        .disabled(!isInteractive)
        .accessibilityLabel(L10n.string("Start Voice Recording"))
        .help(L10n.string("Start Voice Recording"))
    }

    private var legacyVoiceShortcut: KeyCombo {
        KeyBindingStore.shared.combo(for: .toggleVoiceRecording)
    }

    private func handleToggleVoiceRecording() {
        guard isInteractive else { return }
        NotificationCenter.default.post(name: .toggleVoiceRecording, object: nil)
    }

    private func revealInFinder(_ path: String) {
        let url = URL(fileURLWithPath: path)
        NSWorkspace.shared.activateFileViewerSelecting([url])
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

struct StatusBarSeparator: View {
    var body: some View {
        Rectangle()
            .fill(MuxyTheme.border)
            .frame(width: 1)
            .frame(maxHeight: .infinity)
            .accessibilityHidden(true)
    }
}
