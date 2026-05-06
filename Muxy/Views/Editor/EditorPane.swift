import AppKit
import SwiftUI

struct EditorPane: View {
    @Bindable var state: EditorTabState
    let focused: Bool
    let onFocus: () -> Void
    @Environment(AppState.self) private var appState
    @Environment(GhosttyService.self) private var ghostty
    @State private var editorSettings = EditorSettings.shared
    @FocusState private var markdownPreviewFocused: Bool

    var body: some View {
        VStack(spacing: 0) {
            EditorBreadcrumb(state: state)
            Rectangle().fill(MuxyTheme.border).frame(height: 1)
            if state.awaitingLargeFileConfirmation {
                largeFileConfirmation
            } else if state.isLoading {
                loadingView
            } else if let error = state.errorMessage {
                errorView(error)
            } else {
                if state.hasExternalChange {
                    externalChangeBanner
                    Rectangle().fill(MuxyTheme.border).frame(height: 1)
                }
                editorContentLayer
            }
        }
        .background(MuxyTheme.bg)
        .contentShape(Rectangle())
        .simultaneousGesture(TapGesture().onEnded { onFocus() })
        .onReceive(NotificationCenter.default.publisher(for: .findInTerminal)) { _ in
            guard focused else { return }
            if state.isMarkdownFile, state.markdownViewMode == .preview {
                state.markdownViewMode = .code
            }
            if !state.currentSelection.isEmpty {
                state.searchNeedle = state.currentSelection
            }
            state.searchVisible = true
            state.searchFocusVersion += 1
        }
    }

    private var editorContentLayer: some View {
        ZStack(alignment: .topTrailing) {
            editorMainContent

            if state.isIncrementalLoading {
                HStack(spacing: UIMetrics.spacing3) {
                    ProgressView().controlSize(.mini)
                    Text("Loading full file...")
                        .font(.system(size: UIMetrics.fontFootnote))
                        .foregroundStyle(MuxyTheme.fgMuted)
                }
                .padding(.horizontal, UIMetrics.spacing4)
                .padding(.vertical, UIMetrics.spacing3)
                .background(MuxyTheme.bg.opacity(0.92))
                .overlay(
                    RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
                        .stroke(MuxyTheme.border, lineWidth: 1)
                )
                .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
                .padding(.top, UIMetrics.spacing3)
                .padding(.trailing, state.searchVisible && showsCodeEditor ? UIMetrics.scaled(260) : UIMetrics.spacing4)
            }

            if state.searchVisible, showsCodeEditor {
                EditorSearchBar(
                    state: state,
                    onNext: {
                        state.navigateSearch(.next)
                    },
                    onPrevious: {
                        state.navigateSearch(.previous)
                    },
                    onReplace: {
                        state.requestReplaceCurrent()
                    },
                    onReplaceAll: {
                        state.requestReplaceAll()
                    },
                    onClose: {
                        state.searchVisible = false
                        state.editorFocusVersion += 1
                    }
                )
            }
        }
    }

    @ViewBuilder
    private var editorMainContent: some View {
        if state.isMarkdownFile {
            switch state.markdownViewMode {
            case .code:
                codeEditorContainer
            case .preview:
                markdownPreviewContainer
            case .split:
                HSplitView {
                    codeEditorContainer
                    markdownPreviewContainer
                }
            }
        } else {
            codeEditorContainer
        }
    }

    private var codeEditorContainer: some View {
        HStack(spacing: 0) {
            CodeEditorView(
                state: state,
                editorSettings: editorSettings,
                showLineNumbers: editorSettings.showLineNumbers,
                lineWrapping: editorSettings.lineWrapping,
                themeVersion: ghostty.configVersion,
                showsVerticalScroller: true,
                focused: focused,
                searchNeedle: state.searchNeedle,
                searchNavigationVersion: state.searchNavigationVersion,
                searchNavigationDirection: state.searchNavigationDirection,
                searchCaseSensitive: state.searchCaseSensitive,
                searchUseRegex: state.searchUseRegex,
                replaceText: state.replaceText,
                replaceVersion: state.replaceVersion,
                replaceAllVersion: state.replaceAllVersion,
                editorFocusVersion: state.editorFocusVersion,
                onFocus: onFocus
            )
        }
    }

    private var markdownPreviewContainer: some View {
        Group {
            if shouldDelayMarkdownPreview {
                markdownPreviewLoadingView
            } else {
                MarkdownWebView(
                    html: renderedMarkdownHTML,
                    content: renderedMarkdownContent,
                    filePath: state.filePath,
                    projectPath: state.projectPath,
                    palette: markdownPalette,
                    syncScrollRequest: $state.markdownPreviewScrollRequest,
                    syncScrollRequestVersion: state.markdownPreviewScrollRequestVersion,
                    fragmentTarget: state.markdownFragmentTarget,
                    fragmentRequestVersion: state.markdownFragmentRequestVersion,
                    scrollSyncEnabled: usesMarkdownAnchorSync,
                    onScrollReport: { report in
                        state.markdownPreviewMaxScrollTop = report.maxScrollTop
                        state.markdownPreviewViewportHeight = report.clientHeight
                        let map = state.currentMarkdownSyncMap()
                        let output = state.markdownSyncCoordinator.previewDidScroll(scrollTop: report.scrollTop, map: map)
                        state.applyMarkdownSyncOutput(output)
                    },
                    onLayoutChanged: {
                        let map = state.currentMarkdownSyncMap()
                        let output = state.markdownSyncCoordinator.reissueAfterRelayout(map: map)
                        state.applyMarkdownSyncOutput(output)
                    },
                    onAnchorGeometryChanged: { geometries in
                        state.markdownPreviewGeometries = geometries
                    },
                    onOpenInternalLink: { path, fragment in
                        guard let projectID = appState.activeProjectID else { return }
                        appState.openMarkdownLinkTarget(path, projectID: projectID, fragment: fragment)
                    },
                    onReloadFromDisk: { reloadMarkdownFromDisk() }
                )
            }
        }
        .background(Color(nsColor: markdownPalette.background))
        .focusable(focused)
        .focusEffectDisabled()
        .focused($markdownPreviewFocused)
        .onKeyPress(keys: ["e"]) { press in
            guard state.markdownViewMode == .preview else { return .ignored }
            let disallowed: EventModifiers = [.command, .control, .option]
            guard press.modifiers.isDisjoint(with: disallowed) else { return .ignored }
            state.markdownViewMode = press.modifiers.contains(.shift) ? .split : .code
            return .handled
        }
        .onAppear { acquireMarkdownPreviewFocusIfNeeded() }
        .onChange(of: focused) { _, _ in acquireMarkdownPreviewFocusIfNeeded() }
        .onChange(of: state.markdownViewMode) { _, _ in acquireMarkdownPreviewFocusIfNeeded() }
    }

    private func reloadMarkdownFromDisk() {
        guard state.isModified, !state.hasExternalChange else {
            state.reloadFromDisk()
            return
        }
        guard let window = NSApp.keyWindow ?? NSApp.mainWindow else {
            state.reloadFromDisk()
            return
        }
        let alert = NSAlert()
        alert.messageText = "Discard Unsaved Changes?"
        alert.informativeText = "Reloading \(state.fileName) from disk will discard your unsaved changes."
        alert.alertStyle = .warning
        alert.icon = NSApp.applicationIconImage
        alert.addButton(withTitle: "Reload from Disk")
        alert.addButton(withTitle: "Cancel")
        alert.buttons[0].keyEquivalent = "\r"
        alert.buttons[1].keyEquivalent = "\u{1b}"
        alert.beginSheetModal(for: window) { response in
            guard response == .alertFirstButtonReturn else { return }
            state.reloadFromDisk()
        }
    }

    private func acquireMarkdownPreviewFocusIfNeeded() {
        guard focused, state.isMarkdownFile, state.markdownViewMode == .preview else { return }
        if state.suppressInitialFocus {
            state.suppressInitialFocus = false
            return
        }
        markdownPreviewFocused = true
    }

    private var renderedMarkdownContent: String {
        _ = state.previewRefreshVersion
        return state.backingStore?.fullText() ?? ""
    }

    private var renderedMarkdownHTML: String {
        MarkdownRenderer.html(filePath: state.filePath)
    }

    private var markdownPalette: MarkdownRenderer.Palette {
        let palette = EditorThemePalette.active
        return MarkdownRenderer.Palette(
            background: palette.background,
            foreground: palette.foreground,
            accent: palette.accent,
            fontFamilyCSS: editorSettings.resolvedMarkdownPreviewFontFamilyCSS,
            fontScale: editorSettings.markdownPreviewFontScale
        )
    }

    private var usesMarkdownAnchorSync: Bool {
        state.markdownViewMode == .split && state.markdownScrollSyncEnabled && !shouldDelayMarkdownPreview
    }

    private var shouldDelayMarkdownPreview: Bool {
        state.isMarkdownFile && state.isIncrementalLoading
    }

    private var markdownPreviewLoadingView: some View {
        VStack(spacing: UIMetrics.spacing5) {
            ProgressView()
                .controlSize(.small)
            Text("Loading full markdown preview...")
                .font(.system(size: UIMetrics.fontBody))
                .foregroundStyle(MuxyTheme.fgMuted)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(MuxyTheme.bg)
    }

    private var showsCodeEditor: Bool {
        !state.isMarkdownFile || state.markdownViewMode != .preview
    }

    private var externalChangeBanner: some View {
        HStack(spacing: UIMetrics.spacing4) {
            Image(systemName: "exclamationmark.triangle.fill")
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.diffHunkFg)
            Text("This file changed on disk. You have unsaved changes.")
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fg)
            Spacer()
            Button("Reload from Disk") {
                state.reloadFromDisk()
            }
            .controlSize(.small)
            Button("Keep My Changes") {
                state.keepLocalChanges()
            }
            .controlSize(.small)
        }
        .padding(.horizontal, UIMetrics.spacing5)
        .padding(.vertical, UIMetrics.spacing3)
        .background(MuxyTheme.surface)
    }

    private var loadingView: some View {
        VStack {
            Spacer()
            ProgressView().controlSize(.small)
            Spacer()
        }
    }

    private var largeFileConfirmation: some View {
        VStack(spacing: UIMetrics.spacing7) {
            Spacer()
            Image(systemName: "exclamationmark.triangle")
                .font(.system(size: UIMetrics.fontMega))
                .foregroundStyle(MuxyTheme.fgMuted)
            Text("Large File")
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)
            Text("This file is \(formattedLargeFileSize). Large files may slow down the editor.")
                .font(.system(size: UIMetrics.fontBody))
                .foregroundStyle(MuxyTheme.fgMuted)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 360)
            HStack(spacing: UIMetrics.spacing4) {
                Button("Cancel") {
                    state.cancelLargeFileOpen()
                }
                .keyboardShortcut(.cancelAction)
                Button("Open Anyway") {
                    state.confirmLargeFileOpen()
                }
                .keyboardShortcut(.defaultAction)
            }
            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var formattedLargeFileSize: String {
        let formatter = ByteCountFormatter()
        formatter.allowedUnits = [.useMB, .useKB, .useGB]
        formatter.countStyle = .file
        return formatter.string(fromByteCount: state.largeFileSize)
    }

    private func errorView(_ error: String) -> some View {
        VStack {
            Spacer()
            Text(error)
                .font(.system(size: UIMetrics.fontBody))
                .foregroundStyle(MuxyTheme.diffRemoveFg)
            Spacer()
        }
    }
}

private struct EditorMarkdownModePicker: View {
    @Binding var mode: EditorMarkdownViewMode
    @Binding var scrollSyncEnabled: Bool

    var body: some View {
        HStack(spacing: UIMetrics.spacing1) {
            if mode == .split {
                Button {
                    scrollSyncEnabled.toggle()
                } label: {
                    Image(systemName: "arrow.up.and.down")
                        .font(.system(size: UIMetrics.fontCaption, weight: .medium))
                        .foregroundStyle(scrollSyncEnabled ? MuxyTheme.accent : MuxyTheme.fg)
                        .frame(width: UIMetrics.scaled(22), height: UIMetrics.controlSmall)
                        .background(
                            RoundedRectangle(cornerRadius: UIMetrics.radiusSM)
                                .fill(scrollSyncEnabled ? MuxyTheme.surface : Color.clear)
                        )
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .help(scrollSyncEnabled ? "Disable Scroll Sync" : "Enable Scroll Sync")
                .accessibilityLabel(scrollSyncEnabled ? "Disable Markdown Scroll Sync" : "Enable Markdown Scroll Sync")

                Rectangle()
                    .fill(MuxyTheme.border)
                    .frame(width: 1, height: UIMetrics.scaled(14))
                    .padding(.horizontal, UIMetrics.spacing1)
            }
            ForEach(EditorMarkdownViewMode.allCases, id: \.self) { candidate in
                Button {
                    mode = candidate
                } label: {
                    Image(systemName: candidate.symbol)
                        .font(.system(size: UIMetrics.fontCaption, weight: .medium))
                        .frame(width: UIMetrics.scaled(22), height: UIMetrics.controlSmall)
                        .background(
                            RoundedRectangle(cornerRadius: UIMetrics.radiusSM)
                                .fill(mode == candidate ? MuxyTheme.surface : Color.clear)
                        )
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .help(helpText(for: candidate, currentMode: mode))
                .accessibilityLabel("Markdown \(candidate.title) View")
            }
        }
        .padding(UIMetrics.spacing1)
        .background(MuxyTheme.bg)
        .overlay(
            RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
                .stroke(MuxyTheme.border, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
    }

    private func helpText(for candidate: EditorMarkdownViewMode, currentMode: EditorMarkdownViewMode) -> String {
        guard currentMode == .preview else { return candidate.title }
        switch candidate {
        case .code: return "\(candidate.title) (E)"
        case .split: return "\(candidate.title) (⇧E)"
        case .preview: return candidate.title
        }
    }
}

private struct EditorBreadcrumb: View {
    @Bindable var state: EditorTabState

    private var relativePath: String {
        let full = state.filePath
        let base = state.projectPath
        guard full.hasPrefix(base) else { return state.fileName }
        var rel = String(full.dropFirst(base.count))
        if rel.hasPrefix("/") { rel = String(rel.dropFirst()) }
        return rel
    }

    var body: some View {
        HStack(spacing: UIMetrics.spacing2) {
            Image(systemName: "doc.text")
                .font(.system(size: UIMetrics.fontCaption))
                .foregroundStyle(MuxyTheme.fgDim)
            Text(relativePath)
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgMuted)
                .lineLimit(1)
                .truncationMode(.middle)
                .textSelection(.enabled)
            if state.isModified {
                Circle()
                    .fill(MuxyTheme.fg)
                    .frame(width: UIMetrics.scaled(6), height: UIMetrics.scaled(6))
            }
            if state.isReadOnly {
                Label("Read-only", systemImage: "lock.fill")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    .foregroundStyle(MuxyTheme.diffHunkFg)
            }
            Spacer()
            if state.isMarkdownFile {
                EditorMarkdownModePicker(
                    mode: $state.markdownViewMode,
                    scrollSyncEnabled: $state.markdownScrollSyncEnabled
                )
                .padding(.trailing, UIMetrics.spacing3)
            }
            Text("Ln \(state.cursorLine), Col \(state.cursorColumn)")
                .font(.system(size: UIMetrics.fontCaption, design: .monospaced))
                .foregroundStyle(MuxyTheme.fgDim)
        }
        .padding(.horizontal, UIMetrics.spacing5)
        .frame(height: UIMetrics.scaled(32))
        .background(MuxyTheme.bg)
        .accessibilityElement(children: .combine)
        .accessibilityLabel(breadcrumbAccessibilityLabel)
    }

    private var breadcrumbAccessibilityLabel: String {
        var label = relativePath
        if state.isModified { label += ", modified" }
        if state.isReadOnly { label += ", read-only" }
        label += ", Line \(state.cursorLine), Column \(state.cursorColumn)"
        return label
    }
}
