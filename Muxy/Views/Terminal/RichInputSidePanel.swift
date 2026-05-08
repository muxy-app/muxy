import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct RichInputSidePanel: View {
    @Bindable var state: RichInputState
    let worktreeKey: WorktreeKey
    let onDismiss: () -> Void
    let onSubmit: (_ appendReturn: Bool) -> Void

    @AppStorage(RichInputPreferences.fontSizeKey) private var fontSize: Double = RichInputPreferences.defaultFontSize
    @State private var keyMonitor: Any?

    var body: some View {
        VStack(spacing: 0) {
            header
            Rectangle().fill(MuxyTheme.border).frame(height: 1)
            MarkdownTextEditor(
                text: $state.text,
                focusVersion: state.focusVersion,
                configuration: editorConfiguration,
                callbacks: editorCallbacks
            )
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(MuxyTheme.bg)
            if !state.fileAttachments.isEmpty {
                Rectangle().fill(MuxyTheme.border).frame(height: 1)
                AttachmentChipsView(
                    attachments: state.fileAttachments,
                    onRemove: { url in
                        state.fileAttachments.removeAll { $0 == url }
                    }
                )
                .padding(.horizontal, 10)
                .padding(.vertical, 6)
                .background(MuxyTheme.bg)
            }
        }
        .background(MuxyTheme.bg)
        .onDrop(of: [UTType.fileURL, UTType.image], isTargeted: nil) { providers in
            handleDrop(providers: providers)
        }
        .onAppear { installSubmitMonitor() }
        .onDisappear {
            removeSubmitMonitor()
            RichInputDraftStore.shared.flush()
        }
        .onChange(of: state.text) { persistDraft() }
        .onChange(of: state.fileAttachments) { persistDraft() }
        .onChange(of: state.imageAttachments) { persistDraft() }
    }

    private func persistDraft() {
        RichInputDraftStore.shared.scheduleSave(state.draft, for: worktreeKey)
    }

    private var editorConfiguration: MarkdownTextEditor.Configuration {
        MarkdownTextEditor.Configuration(
            font: .systemFont(ofSize: clampedFontSize),
            insets: NSSize(width: 12, height: 10),
            lineWrapping: true,
            grabsFirstResponderOnAppear: true
        )
    }

    private var editorCallbacks: MarkdownTextEditor.Callbacks {
        MarkdownTextEditor.Callbacks(
            onSubmit: { onSubmit(true) },
            onPasteImageData: { data in
                guard let url = RichInputImageStorage.write(imageData: data) else { return }
                insertImagePlaceholder(for: url)
            },
            onPasteFileURL: { url in
                guard !state.fileAttachments.contains(url) else { return }
                state.fileAttachments.append(url)
            }
        )
    }

    private var clampedFontSize: CGFloat {
        let bounded = min(max(fontSize, RichInputPreferences.minFontSize), RichInputPreferences.maxFontSize)
        return CGFloat(bounded)
    }

    private var header: some View {
        HStack(spacing: 8) {
            Image(systemName: "keyboard")
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
            Text("Rich Input")
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)
            Spacer(minLength: 8)
            Button(action: onDismiss) {
                Image(systemName: "xmark")
                    .font(.system(size: 11, weight: .semibold))
            }
            .buttonStyle(RichInputToolbarButtonStyle())
            .accessibilityLabel("Close Rich Input")
            .help("Close Rich Input")
        }
        .padding(.horizontal, 10)
        .frame(height: 32)
        .background(MuxyTheme.bg)
    }

    private func increaseFontSize() {
        fontSize = min(RichInputPreferences.maxFontSize, fontSize + RichInputPreferences.fontStep)
    }

    private func decreaseFontSize() {
        fontSize = max(RichInputPreferences.minFontSize, fontSize - RichInputPreferences.fontStep)
    }

    private func insertImagePlaceholder(for url: URL) {
        let placeholder = state.nextImagePlaceholder(for: url)
        state.text.append(placeholder)
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
                    Task { @MainActor in
                        if !state.fileAttachments.contains(url) {
                            state.fileAttachments.append(url)
                        }
                    }
                }
                consumed = true
                continue
            }
            if provider.hasItemConformingToTypeIdentifier(UTType.image.identifier) {
                provider.loadDataRepresentation(forTypeIdentifier: UTType.image.identifier) { data, _ in
                    guard let data, let url = RichInputImageStorage.write(imageData: data) else { return }
                    Task { @MainActor in
                        insertImagePlaceholder(for: url)
                    }
                }
                consumed = true
            }
        }
        return consumed
    }

    private func installSubmitMonitor() {
        guard keyMonitor == nil else { return }
        keyMonitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown]) { event in
            guard NSApp.keyWindow?.firstResponder is MarkdownEditingTextView else { return event }
            let store = KeyBindingStore.shared
            if store.combo(for: .submitRichInput).matches(event: event) {
                Task { @MainActor in onSubmit(true) }
                return nil
            }
            if store.combo(for: .submitRichInputWithoutReturn).matches(event: event) {
                Task { @MainActor in onSubmit(false) }
                return nil
            }
            let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
            guard flags.subtracting(.shift) == .command else { return event }
            switch event.charactersIgnoringModifiers?.lowercased() {
            case "=",
                 "+":
                Task { @MainActor in increaseFontSize() }
                return nil
            case "-":
                Task { @MainActor in decreaseFontSize() }
                return nil
            default:
                return event
            }
        }
    }

    private func removeSubmitMonitor() {
        if let monitor = keyMonitor {
            NSEvent.removeMonitor(monitor)
            keyMonitor = nil
        }
    }
}

private struct AttachmentChipsView: View {
    let attachments: [URL]
    let onRemove: (URL) -> Void

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                ForEach(attachments, id: \.self) { url in
                    AttachmentChip(url: url, onRemove: { onRemove(url) })
                }
            }
        }
    }
}

private struct AttachmentChip: View {
    let url: URL
    let onRemove: () -> Void

    private var isImage: Bool {
        guard let utType = try? url.resourceValues(forKeys: [.contentTypeKey]).contentType else {
            return false
        }
        return utType.conforms(to: .image)
    }

    var body: some View {
        HStack(spacing: 4) {
            Image(systemName: isImage ? "photo" : "doc")
                .font(.system(size: 10))
                .foregroundStyle(MuxyTheme.fgMuted)
            Text(url.lastPathComponent)
                .font(.system(size: 11))
                .foregroundStyle(MuxyTheme.fg)
                .lineLimit(1)
                .truncationMode(.middle)
            Button(action: onRemove) {
                Image(systemName: "xmark")
                    .font(.system(size: 9, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Remove attachment")
        }
        .padding(.horizontal, 7)
        .padding(.vertical, 3)
        .background(MuxyTheme.surface)
        .clipShape(Capsule())
        .overlay(Capsule().strokeBorder(MuxyTheme.border, lineWidth: 1))
    }
}
