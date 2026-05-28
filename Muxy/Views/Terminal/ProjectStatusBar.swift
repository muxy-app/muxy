import AppKit
import SwiftUI

struct ProjectStatusBar: View {
    struct StatusContext: Equatable {
        let path: String
        let worktreeName: String?
        let branch: String?
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
    @AppStorage(AIUsageSettingsStore.usageEnabledKey) private var usageEnabled = false
    @AppStorage(AIUsageSettingsStore.usageDisplayModeKey) private var usageDisplayModeRaw = AIUsageSettingsStore
        .defaultUsageDisplayMode.rawValue
    @AppStorage(AIUsageSettingsStore.sidebarPreviewProviderIDKey) private var pinnedPreviewProviderID: String = ""
    @State private var showAIUsagePopover = false
    private let usageService = AIUsageService.shared
    private let usageRefreshTimer = Timer.publish(every: 60, on: .main, in: .common).autoconnect()

    private var richInputShortcutLabel: String {
        KeyBindingStore.shared.combo(for: .toggleRichInput).displayString
    }

    private var voiceShortcutLabel: String {
        KeyBindingStore.shared.combo(for: .toggleVoiceRecording).displayString
    }

    private var usageDisplayMode: AIUsageDisplayMode {
        AIUsageDisplayMode(rawValue: usageDisplayModeRaw) ?? AIUsageSettingsStore.defaultUsageDisplayMode
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
        .task {
            await usageService.refreshIfNeeded()
        }
        .onReceive(usageRefreshTimer) { _ in
            Task { await usageService.refreshIfNeeded() }
        }
        .onReceive(NotificationCenter.default.publisher(for: .toggleAIUsage)) { _ in
            guard usageEnabled else { return }
            showAIUsagePopover.toggle()
        }
        .onChange(of: usageEnabled) { _, enabled in
            if !enabled { showAIUsagePopover = false }
        }
    }

    private var leftSide: some View {
        HStack(spacing: 8) {
            if let statusContext {
                pathButton(statusContext.path)
                separator
                if let worktreeName = statusContext.worktreeName {
                    worktreeLabel(worktreeName)
                    separator
                }
                if let branch = statusContext.branch {
                    branchLabel(branch)
                    separator
                }
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
            if usageEnabled {
                separator
                aiUsageItem
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
        return StatusContext(
            path: path,
            worktreeName: activeWorktree?.name,
            branch: nonEmpty(activePane?.branchObserver.branch) ?? nonEmpty(activeWorktree?.branch)
        )
    }

    private static func nonEmpty(_ value: String?) -> String? {
        guard let trimmed = value?.trimmingCharacters(in: .whitespacesAndNewlines),
              !trimmed.isEmpty
        else { return nil }
        return trimmed
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

    private func worktreeLabel(_ worktreeName: String) -> some View {
        HStack(spacing: 4) {
            Image(systemName: "square.stack.3d.up")
                .font(.system(size: 10, weight: .semibold))
            Text(worktreeName)
                .font(.system(size: 11, weight: .medium))
                .lineLimit(1)
                .truncationMode(.tail)
        }
        .foregroundStyle(MuxyTheme.fgMuted)
        .help("Worktree: \(worktreeName)")
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

    private func extensionItem(binding: ExtensionStore.StatusBarItemBinding) -> some View {
        Button {
            onTriggerExtensionCommand?(binding)
        } label: {
            HStack(spacing: 4) {
                ExtensionIconView(
                    icon: binding.item.icon,
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
        }
        .buttonStyle(.plain)
        .help(binding.item.tooltip ?? binding.item.id)
        .accessibilityLabel(binding.item.tooltip ?? binding.item.id)
    }

    private var previewProviderDisplay: (percent: Int, iconName: String)? {
        guard let selection = usageService.previewSelection(pinnedRawValue: pinnedPreviewProviderID),
              case .available = selection.snapshot.state
        else { return nil }

        let snapshot = selection.snapshot
        let rowPercent = selection.row?.percent
        let usedPercent = max(0, min(100, rowPercent ?? snapshot.rows.compactMap(\.percent).max() ?? 0))
        let displayPercent: Double = switch usageDisplayMode {
        case .used:
            usedPercent
        case .remaining:
            max(0, min(100, 100 - usedPercent))
        }

        return (Int(displayPercent.rounded()), snapshot.providerIconName)
    }

    private var previewProviderPercentLabel: String? {
        guard let display = previewProviderDisplay else { return nil }
        return "\(max(0, min(100, display.percent)))%"
    }

    private var aiUsageItem: some View {
        Button {
            showAIUsagePopover.toggle()
        } label: {
            HStack(spacing: 4) {
                if let display = previewProviderDisplay {
                    ProviderIconView(iconName: display.iconName, size: 11, style: .monochrome(MuxyTheme.fgMuted))
                } else {
                    Image(systemName: "sparkles")
                        .font(.system(size: 10, weight: .semibold))
                }
                if let percentLabel = previewProviderPercentLabel {
                    Text(percentLabel)
                        .font(.system(size: 11, weight: .medium))
                        .lineLimit(1)
                }
            }
            .foregroundStyle(MuxyTheme.fgMuted)
        }
        .buttonStyle(.plain)
        .popover(isPresented: $showAIUsagePopover) {
            AIUsagePanel(
                snapshots: usageService.snapshots,
                isRefreshing: usageService.isRefreshing,
                lastRefreshDate: usageService.lastRefreshDate,
                onRefresh: refreshUsage
            )
        }
        .help("AI Usage (\(KeyBindingStore.shared.combo(for: .toggleAIUsage).displayString))")
        .accessibilityLabel("AI Usage")
    }

    private func refreshUsage() {
        Task { await usageService.refresh(force: true) }
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
