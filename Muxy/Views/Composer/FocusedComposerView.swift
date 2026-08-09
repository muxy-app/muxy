import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct FocusedComposerView: View {
    @Bindable var state: RichInputState
    @Bindable var voice: ComposerVoiceState
    let worktreeKey: WorktreeKey
    let projectName: String
    let worktreeName: String
    let languageIdentifier: String
    let availableSize: CGSize
    let presentationMode: RichInputPresentationMode
    @Binding var broadcasts: Bool
    let onSubmit: (_ appendReturn: Bool, _ selectedText: String?) -> Void
    let onChangePresentationMode: (RichInputPresentationMode) -> Void
    let onClose: () -> Void

    @State private var editorSettings = EditorSettings.shared
    @AppStorage(RichInputPreferences.fontSizeKey) private var fontSize = RichInputPreferences.defaultFontSize
    @AppStorage(ComposerLayoutPreferences.widthKey) private var storedWidth = ComposerLayoutPreferences.defaultWidth
    @AppStorage(ComposerLayoutPreferences.heightKey) private var storedHeight = ComposerLayoutPreferences.defaultHeight
    @AppStorage(ComposerLayoutPreferences.expandedKey) private var isExpanded = ComposerLayoutPreferences.defaultExpanded
    @State private var insertion: RichInputTextEditor.Insertion?
    @State private var submission: RichInputTextEditor.Submission?

    var body: some View {
        Group {
            switch presentationMode {
            case .panel:
                panelLayout
            case .floating:
                floatingLayout
            }
        }
        .onDrop(of: [UTType.fileURL, UTType.image], isTargeted: nil, perform: handleDrop)
        .onChange(of: state.text) { persistDraft() }
        .onChange(of: state.fileAttachments) { persistDraft() }
        .onChange(of: state.imageAttachments) { persistDraft() }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(L10n.string("Composer"))
    }

    private var floatingLayout: some View {
        VStack(spacing: 0) {
            metadata
            inputContent
        }
        .padding(UIMetrics.scaled(14))
        .background {
            RoundedRectangle(cornerRadius: UIMetrics.scaled(18))
                .fill(MuxyTheme.bg)
                .overlay {
                    RoundedRectangle(cornerRadius: UIMetrics.scaled(18))
                        .fill(MuxyTheme.surface)
                }
        }
        .overlay(resizeEdges)
        .frame(width: UIMetrics.scaled(layoutSize.width), height: UIMetrics.scaled(layoutSize.height))
        .shadow(color: .black.opacity(0.45), radius: UIMetrics.scaled(36), y: UIMetrics.scaled(18))
    }

    private var panelLayout: some View {
        VStack(spacing: 0) {
            inputContent
        }
        .padding(UIMetrics.scaled(14))
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(MuxyTheme.bg)
    }

    @ViewBuilder
    private var inputContent: some View {
        editor
        attachments
        actions
    }

    private var metadata: some View {
        HStack(spacing: UIMetrics.spacing3) {
            Circle()
                .fill(MuxyTheme.diffAddFg)
                .frame(width: UIMetrics.scaled(6), height: UIMetrics.scaled(6))
            Text(L10n.resource("\(projectName) / \(worktreeName)"))
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
                .lineLimit(1)
            Text(L10n.resource("· active pane"))
                .font(.system(size: UIMetrics.fontCaption))
                .foregroundStyle(MuxyTheme.fgDim)
            Spacer(minLength: UIMetrics.spacing3)
            Button(action: toggleExpanded) {
                Image(systemName: isExpanded
                    ? "arrow.down.right.and.arrow.up.left"
                    : "arrow.up.left.and.arrow.down.right"
                )
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                .frame(width: UIMetrics.controlSmall, height: UIMetrics.controlSmall)
            }
            .buttonStyle(.plain)
            .foregroundStyle(MuxyTheme.fgDim)
            .accessibilityLabel(expandLabel)
            .help(expandLabel)
            Button(action: onClose) {
                Image(systemName: "xmark")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    .frame(width: UIMetrics.controlSmall, height: UIMetrics.controlSmall)
            }
            .buttonStyle(.plain)
            .foregroundStyle(MuxyTheme.fgDim)
            .accessibilityLabel(L10n.string("Close Composer"))
        }
        .frame(height: UIMetrics.scaled(22))
    }

    private var expandLabel: String {
        isExpanded ? L10n.string("Collapse Composer") : L10n.string("Expand Composer")
    }

    private var resizeEdges: some View {
        ZStack {
            HStack(spacing: 0) {
                widthEdge(.leading)
                Spacer(minLength: 0)
                widthEdge(.trailing)
            }
            VStack(spacing: 0) {
                Spacer(minLength: 0)
                ComposerResizeEdge(
                    edge: .bottom,
                    captureAnchor: { layoutSize.height },
                    apply: applyHeight
                )
            }
        }
    }

    private func widthEdge(_ edge: ResizeHandle.Edge) -> some View {
        ComposerResizeEdge(
            edge: edge,
            captureAnchor: { layoutSize.width },
            apply: applyWidth
        )
    }

    private var editor: some View {
        RichInputTextEditor(
            text: $state.text,
            focusVersion: state.focusVersion,
            isDictating: voice.recorder.isRecording,
            isSubmissionEnabled: voice.canSubmit,
            insertion: insertion,
            submission: submission,
            configuration: editorConfiguration,
            callbacks: editorCallbacks
        )
        .padding(.bottom, voice.showsFeedback ? UIMetrics.scaled(30) : 0)
        .frame(maxHeight: .infinity)
        .background(MuxyTheme.bg.opacity(0.001))
        .overlay(alignment: .topLeading) {
            if state.text.isEmpty {
                Text(L10n.resource("Type or speak…"))
                    .font(.system(size: clampedFontSize))
                    .foregroundStyle(MuxyTheme.fgMuted.opacity(0.55))
                    .padding(.horizontal, UIMetrics.spacing1)
                    .padding(.vertical, UIMetrics.spacing6)
                    .allowsHitTesting(false)
            }
        }
        .overlay(alignment: .bottomLeading) {
            if voice.showsFeedback {
                voiceFeedback
            }
        }
    }

    private var voiceFeedback: some View {
        HStack(spacing: UIMetrics.spacing4) {
            if let errorMessage = voice.errorMessage {
                Image(systemName: "exclamationmark.circle.fill")
                    .foregroundStyle(MuxyTheme.diffRemoveFg)
                Text(errorMessage)
                    .foregroundStyle(MuxyTheme.diffRemoveFg)
                    .lineLimit(1)
                    .truncationMode(.tail)
                Spacer(minLength: UIMetrics.spacing2)
                Button(action: voice.dismissError) {
                    Image(systemName: "xmark")
                        .font(.system(size: UIMetrics.fontMicro, weight: .semibold))
                }
                .buttonStyle(.plain)
                .foregroundStyle(MuxyTheme.fgDim)
                .accessibilityLabel(L10n.string("Dismiss Dictation Error"))
            } else {
                Text(
                    voice.recorder.isPaused
                        ? L10n.resource("PAUSED")
                        : L10n.resource("● LIVE")
                )
                .font(.system(size: UIMetrics.fontXS, weight: .bold, design: .monospaced))
                .foregroundStyle(voice.recorder.isPaused ? MuxyTheme.warning : MuxyTheme.diffRemoveFg)
                VoiceLevelMeter(level: voice.recorder.level, isPaused: voice.recorder.isPaused)
                Text(voiceTranscript)
                    .foregroundStyle(MuxyTheme.fg)
                    .lineLimit(1)
                    .truncationMode(.head)
                Spacer(minLength: UIMetrics.spacing2)
                Text(VoiceRecordingPanel.formatElapsed(voice.recorder.elapsed))
                    .font(.system(size: UIMetrics.fontXS, design: .monospaced))
                    .foregroundStyle(MuxyTheme.fgDim)
            }
        }
        .font(.system(size: UIMetrics.fontCaption))
        .frame(height: UIMetrics.scaled(26))
        .padding(.horizontal, UIMetrics.spacing1)
    }

    private var voiceTranscript: String {
        if voice.isStarting {
            return L10n.string("Preparing on-device dictation…")
        }
        if voice.recorder.transcript.isEmpty {
            return L10n.string("Listening…")
        }
        return voice.recorder.transcript
    }

    @ViewBuilder
    private var attachments: some View {
        if !state.fileAttachments.isEmpty {
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: UIMetrics.spacing3) {
                    ForEach(state.fileAttachments, id: \.self) { url in
                        ComposerAttachmentChip(url: url) {
                            state.fileAttachments.removeAll { $0 == url }
                        }
                    }
                }
            }
            .frame(height: UIMetrics.scaled(28))
            .padding(.horizontal, UIMetrics.spacing1)
        }
    }

    private var actions: some View {
        HStack(spacing: UIMetrics.spacing2) {
            ComposerIconButton(symbol: "plus", label: L10n.string("Attach File"), action: chooseAttachments)
            ComposerIconButton(
                symbol: voice.recorder.isRecording ? "stop.fill" : "mic",
                label: voice.recorder.isRecording
                    ? L10n.string("Finish Dictation")
                    : L10n.string("Start Dictation"),
                isActive: voice.isBusy,
                activeColor: MuxyTheme.diffRemoveFg,
                action: toggleVoice
            )
            Menu {
                if voice.recorder.isRecording {
                    Button(
                        voice.recorder.isPaused
                            ? L10n.string("Resume Dictation")
                            : L10n.string("Pause Dictation")
                    ) {
                        voice.togglePause()
                    }
                    Button(L10n.string("Cancel Dictation")) {
                        voice.cancel()
                    }
                    Divider()
                }
                Button(
                    broadcasts
                        ? L10n.string("Send to Active Pane")
                        : L10n.string("Send to All Split Panes")
                ) {
                    broadcasts.toggle()
                }
                Button(L10n.string("Send Without Enter")) {
                    requestSubmission(appendReturn: false)
                }
                .disabled(!voice.canSubmit)
                Divider()
                Button(presentationModeSwitchLabel) {
                    onChangePresentationMode(presentationMode == .panel ? .floating : .panel)
                }
                if presentationMode == .floating {
                    Button(L10n.string("Reset Composer Size"), action: resetSize)
                }
            } label: {
                Image(systemName: "ellipsis")
                    .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                    .frame(width: UIMetrics.controlLarge, height: UIMetrics.controlLarge)
                    .contentShape(Rectangle())
            }
            .menuStyle(.borderlessButton)
            .menuIndicator(.hidden)
            .fixedSize()
            .foregroundStyle(MuxyTheme.fgMuted)
            .accessibilityLabel(L10n.string("More Composer Actions"))

            Text(
                broadcasts
                    ? L10n.resource("All split panes")
                    : L10n.resource("Active pane")
            )
            .font(.system(size: UIMetrics.fontXS))
            .foregroundStyle(MuxyTheme.fgDim)
            .lineLimit(1)

            Spacer(minLength: UIMetrics.spacing4)

            Button {
                requestSubmission(appendReturn: true)
            } label: {
                Image(systemName: "arrow.up")
                    .font(.system(size: UIMetrics.fontBody, weight: .bold))
                    .foregroundStyle(MuxyTheme.accentForeground)
                    .frame(width: UIMetrics.controlLarge, height: UIMetrics.controlLarge)
                    .background(MuxyTheme.accent, in: RoundedRectangle(cornerRadius: UIMetrics.radiusXL))
            }
            .buttonStyle(.plain)
            .disabled(!voice.canSubmit)
            .accessibilityLabel(L10n.string("Send"))
        }
        .padding(.top, UIMetrics.spacing4)
    }

    private var presentationModeSwitchLabel: String {
        switch presentationMode {
        case .panel: L10n.string("Use Floating Composer")
        case .floating: L10n.string("Use Composer Panel")
        }
    }

    private var editorConfiguration: RichInputTextEditor.Configuration {
        RichInputTextEditor.Configuration(
            font: resolvedFont,
            insets: NSSize(width: UIMetrics.spacing1, height: UIMetrics.spacing6),
            lineWrapping: true,
            grabsFirstResponderOnAppear: true,
            lineHeightMultiplier: editorSettings.richInputLineHeightMultiplier
        )
    }

    private var editorCallbacks: RichInputTextEditor.Callbacks {
        RichInputTextEditor.Callbacks(
            onSubmit: { selectedText in submitIfPossible(appendReturn: true, selectedText: selectedText) },
            onSubmitWithoutReturn: { selectedText in
                submitIfPossible(appendReturn: false, selectedText: selectedText)
            },
            onFinishDictation: finishDictation,
            onIncreaseFontSize: increaseFontSize,
            onDecreaseFontSize: decreaseFontSize,
            onPasteImageData: { data in
                guard let url = RichInputImageStorage.write(imageData: data) else { return }
                insertImagePlaceholder(for: url)
            },
            onPasteFileURL: addAttachment
        )
    }

    private var layoutSize: CGSize {
        ComposerLayoutPreferences.size(
            width: storedWidth,
            height: storedHeight,
            isExpanded: isExpanded,
            available: logicalAvailableSize
        )
    }

    private var logicalAvailableSize: CGSize {
        CGSize(
            width: UIMetrics.unscaled(availableSize.width),
            height: UIMetrics.unscaled(availableSize.height)
        )
    }

    private func applyWidth(_ width: CGFloat) {
        detachFromExpanded()
        storedWidth = Double(ComposerLayoutPreferences.clampedWidth(width, available: logicalAvailableSize))
    }

    private func applyHeight(_ height: CGFloat) {
        detachFromExpanded()
        storedHeight = Double(ComposerLayoutPreferences.clampedHeight(height, available: logicalAvailableSize))
    }

    private func detachFromExpanded() {
        guard isExpanded else { return }
        let current = layoutSize
        storedWidth = Double(current.width)
        storedHeight = Double(current.height)
        isExpanded = false
    }

    private func toggleExpanded() {
        withAnimation(.easeOut(duration: 0.14)) {
            isExpanded.toggle()
        }
    }

    private func resetSize() {
        withAnimation(.easeOut(duration: 0.14)) {
            isExpanded = ComposerLayoutPreferences.defaultExpanded
            storedWidth = ComposerLayoutPreferences.defaultWidth
            storedHeight = ComposerLayoutPreferences.defaultHeight
        }
    }

    private var clampedFontSize: CGFloat {
        CGFloat(min(max(fontSize, RichInputPreferences.minFontSize), RichInputPreferences.maxFontSize))
    }

    private var resolvedFont: NSFont {
        NSFont(name: editorSettings.richInputFontFamily, size: clampedFontSize)
            ?? .monospacedSystemFont(ofSize: clampedFontSize, weight: .regular)
    }

    private func persistDraft() {
        RichInputDraftStore.shared.scheduleSave(state.draft, for: worktreeKey)
    }

    private func requestSubmission(appendReturn: Bool) {
        guard voice.canSubmit else { return }
        submission = RichInputTextEditor.Submission(id: UUID(), appendReturn: appendReturn)
    }

    private func submitIfPossible(appendReturn: Bool, selectedText: String?) {
        guard voice.canSubmit else { return }
        onSubmit(appendReturn, selectedText)
    }

    private func toggleVoice() {
        if voice.recorder.isRecording {
            finishDictation()
            return
        }
        if voice.isStarting {
            voice.cancel()
            return
        }
        voice.start(languageIdentifier: languageIdentifier)
    }

    private func finishDictation() {
        guard let transcript = voice.finish() else { return }
        insertion = RichInputTextEditor.Insertion(id: UUID(), text: transcript)
    }

    private func chooseAttachments() {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = true
        panel.canChooseDirectories = false
        panel.begin { response in
            guard response == .OK else { return }
            Task { @MainActor in
                panel.urls.forEach(addAttachment)
            }
        }
    }

    private func addAttachment(_ url: URL) {
        guard !state.fileAttachments.contains(url) else { return }
        state.fileAttachments.append(url)
    }

    private func insertImagePlaceholder(for url: URL) {
        state.text.append(state.nextImagePlaceholder(for: url))
    }

    private func increaseFontSize() {
        fontSize = min(RichInputPreferences.maxFontSize, fontSize + RichInputPreferences.fontStep)
    }

    private func decreaseFontSize() {
        fontSize = max(RichInputPreferences.minFontSize, fontSize - RichInputPreferences.fontStep)
    }

    private func handleDrop(providers: [NSItemProvider]) -> Bool {
        var consumed = false
        for provider in providers {
            if provider.hasItemConformingToTypeIdentifier(UTType.fileURL.identifier) {
                provider.loadItem(forTypeIdentifier: UTType.fileURL.identifier, options: nil) { item, _ in
                    let url: URL? = if let url = item as? URL {
                        url
                    } else if let data = item as? Data {
                        URL(dataRepresentation: data, relativeTo: nil)
                    } else {
                        nil
                    }
                    guard let url else { return }
                    Task { @MainActor in addAttachment(url) }
                }
                consumed = true
                continue
            }
            if provider.hasItemConformingToTypeIdentifier(UTType.image.identifier) {
                provider.loadDataRepresentation(forTypeIdentifier: UTType.image.identifier) { data, _ in
                    guard let data, let url = RichInputImageStorage.write(imageData: data) else { return }
                    Task { @MainActor in insertImagePlaceholder(for: url) }
                }
                consumed = true
            }
        }
        return consumed
    }
}

private struct VoiceLevelMeter: View {
    let level: Float
    let isPaused: Bool

    var body: some View {
        HStack(spacing: UIMetrics.spacing1) {
            ForEach(1 ... 5, id: \.self) { index in
                Capsule()
                    .fill(isActive(index) ? MuxyTheme.diffRemoveFg : MuxyTheme.fgDim)
                    .frame(width: UIMetrics.scaled(2), height: UIMetrics.scaled(CGFloat(4 + index * 2)))
            }
        }
        .frame(height: UIMetrics.iconLG)
        .accessibilityHidden(true)
    }

    private func isActive(_ index: Int) -> Bool {
        !isPaused && level >= Float(index) / 5
    }
}

private struct ComposerAttachmentChip: View {
    let url: URL
    let onRemove: () -> Void

    var body: some View {
        HStack(spacing: UIMetrics.spacing2) {
            Image(systemName: "doc")
                .font(.system(size: UIMetrics.fontXS))
            Text(url.lastPathComponent)
                .lineLimit(1)
                .truncationMode(.middle)
            Button(action: onRemove) {
                Image(systemName: "xmark")
                    .font(.system(size: UIMetrics.fontMicro, weight: .semibold))
            }
            .buttonStyle(.plain)
            .accessibilityLabel(L10n.string("Remove \(url.lastPathComponent)"))
        }
        .font(.system(size: UIMetrics.fontXS))
        .foregroundStyle(MuxyTheme.fgMuted)
        .padding(.horizontal, UIMetrics.spacing4)
        .padding(.vertical, UIMetrics.spacing2)
        .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
    }
}

private struct ComposerIconButton: View {
    let symbol: String
    let label: String
    var isActive = false
    var activeColor = MuxyTheme.accent
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                .foregroundStyle(isActive ? Color.white : MuxyTheme.fgMuted)
                .frame(width: UIMetrics.controlLarge, height: UIMetrics.controlLarge)
                .background(isActive ? activeColor : .clear, in: RoundedRectangle(cornerRadius: UIMetrics.radiusXL))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
        .help(label)
    }
}
