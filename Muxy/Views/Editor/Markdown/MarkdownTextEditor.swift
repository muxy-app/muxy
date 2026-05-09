import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct MarkdownTextEditor: NSViewRepresentable {
    struct Configuration {
        var font: NSFont = .systemFont(ofSize: 13)
        var insets: NSSize = .init(width: 8, height: 8)
        var allowsUndo: Bool = true
        var lineWrapping: Bool = true
        var grabsFirstResponderOnAppear: Bool = false
    }

    struct Callbacks {
        var onSubmit: (() -> Void)?
        var onSubmitWithoutReturn: (() -> Void)?
        var onIncreaseFontSize: (() -> Void)?
        var onDecreaseFontSize: (() -> Void)?
        var onPasteImageData: ((Data) -> Void)?
        var onPasteFileURL: ((URL) -> Void)?
        var onContentHeightChange: ((CGFloat) -> Void)?
    }

    @Binding var text: String
    var focusVersion: Int = 0
    var configuration: Configuration = .init()
    var callbacks: Callbacks = .init()

    func makeCoordinator() -> Coordinator {
        Coordinator(parent: self)
    }

    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSScrollView()
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = !configuration.lineWrapping
        scrollView.borderType = .noBorder
        scrollView.drawsBackground = false
        scrollView.autohidesScrollers = true
        scrollView.contentView.drawsBackground = false

        let containerWidth: CGFloat = configuration.lineWrapping ? 0 : CGFloat.greatestFiniteMagnitude
        let textContainer = NSTextContainer(containerSize: NSSize(
            width: containerWidth,
            height: CGFloat.greatestFiniteMagnitude
        ))
        textContainer.widthTracksTextView = configuration.lineWrapping
        textContainer.lineFragmentPadding = 0

        let layoutManager = NSLayoutManager()
        layoutManager.addTextContainer(textContainer)

        let textStorage = NSTextStorage()
        textStorage.addLayoutManager(layoutManager)

        let textView = MarkdownEditingTextView(
            frame: NSRect(origin: .zero, size: scrollView.contentSize),
            textContainer: textContainer
        )
        textView.minSize = NSSize(width: 0, height: 0)
        textView.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        textView.isVerticallyResizable = true
        textView.isHorizontallyResizable = !configuration.lineWrapping
        textView.autoresizingMask = configuration.lineWrapping ? [.width] : []
        textView.textContainerInset = configuration.insets
        textView.isEditable = true
        textView.isSelectable = true
        textView.isRichText = false
        textView.allowsUndo = configuration.allowsUndo
        textView.drawsBackground = false
        textView.isAutomaticQuoteSubstitutionEnabled = false
        textView.isAutomaticDashSubstitutionEnabled = false
        textView.isAutomaticTextReplacementEnabled = false
        textView.isAutomaticSpellingCorrectionEnabled = false
        textView.isAutomaticTextCompletionEnabled = false
        textView.isAutomaticLinkDetectionEnabled = false
        textView.smartInsertDeleteEnabled = false
        textView.isGrammarCheckingEnabled = false
        textView.isContinuousSpellCheckingEnabled = false
        textView.font = configuration.font
        textView.textColor = MarkdownTextEditor.defaultForeground()
        textView.insertionPointColor = MarkdownTextEditor.defaultForeground()
        textView.delegate = context.coordinator
        textView.onSubmit = { [weak coordinator = context.coordinator] in
            coordinator?.parent.callbacks.onSubmit?()
        }
        textView.onSubmitWithoutReturn = { [weak coordinator = context.coordinator] in
            coordinator?.parent.callbacks.onSubmitWithoutReturn?()
        }
        textView.onIncreaseFontSize = { [weak coordinator = context.coordinator] in
            coordinator?.parent.callbacks.onIncreaseFontSize?()
        }
        textView.onDecreaseFontSize = { [weak coordinator = context.coordinator] in
            coordinator?.parent.callbacks.onDecreaseFontSize?()
        }
        textView.onPasteImageData = { [weak coordinator = context.coordinator] data in
            coordinator?.parent.callbacks.onPasteImageData?(data)
        }
        textView.onPasteFileURL = { [weak coordinator = context.coordinator] url in
            coordinator?.parent.callbacks.onPasteFileURL?(url)
        }

        if textView.string != text {
            textView.string = text
        }

        textView.pendingFocusGrab = configuration.grabsFirstResponderOnAppear
        scrollView.documentView = textView
        context.coordinator.textView = textView
        context.coordinator.lastFocusVersion = focusVersion
        context.coordinator.applyHighlighting()
        DispatchQueue.main.async { [weak coordinator = context.coordinator] in
            coordinator?.reportContentHeight()
        }
        return scrollView
    }

    func updateNSView(_ scrollView: NSScrollView, context: Context) {
        guard let textView = scrollView.documentView as? MarkdownEditingTextView else { return }
        context.coordinator.parent = self
        if textView.string != text {
            textView.string = text
            context.coordinator.applyHighlighting()
        }
        textView.font = configuration.font
        textView.textContainerInset = configuration.insets
        textView.textColor = MarkdownTextEditor.defaultForeground()
        textView.insertionPointColor = MarkdownTextEditor.defaultForeground()
        if context.coordinator.lastFocusVersion != focusVersion {
            context.coordinator.lastFocusVersion = focusVersion
            textView.grabFirstResponder()
        }
        DispatchQueue.main.async { [weak coordinator = context.coordinator] in
            coordinator?.reportContentHeight()
        }
    }

    @MainActor
    private static func defaultForeground() -> NSColor {
        NSColor(MuxyTheme.fg)
    }

    @MainActor
    final class Coordinator: NSObject, NSTextViewDelegate {
        var parent: MarkdownTextEditor
        weak var textView: MarkdownEditingTextView?
        var lastFocusVersion: Int = -1
        private var lastReportedHeight: CGFloat = -1

        init(parent: MarkdownTextEditor) {
            self.parent = parent
        }

        func textDidChange(_ notification: Notification) {
            guard let textView = notification.object as? NSTextView else { return }
            parent.text = textView.string
            applyHighlighting()
            reportContentHeight()
        }

        func reportContentHeight() {
            guard let textView,
                  let layoutManager = textView.layoutManager,
                  let textContainer = textView.textContainer
            else { return }
            layoutManager.ensureLayout(for: textContainer)
            let usedRect = layoutManager.usedRect(for: textContainer)
            let inset = textView.textContainerInset.height
            let height = ceil(usedRect.height + inset * 2)
            guard abs(height - lastReportedHeight) > 0.5 else { return }
            lastReportedHeight = height
            parent.callbacks.onContentHeightChange?(height)
        }

        func applyHighlighting() {
            guard let textView,
                  let layoutManager = textView.layoutManager
            else { return }
            let fullText = textView.string as NSString
            let fullRange = NSRange(location: 0, length: fullText.length)
            layoutManager.removeTemporaryAttribute(.foregroundColor, forCharacterRange: fullRange)
            layoutManager.removeTemporaryAttribute(.strikethroughStyle, forCharacterRange: fullRange)
            guard fullRange.length > 0 else { return }

            var lineStart = 0
            var insideFence = false
            while lineStart < fullText.length {
                var lineEnd = 0
                var contentsEnd = 0
                fullText.getLineStart(nil, end: &lineEnd, contentsEnd: &contentsEnd, for: NSRange(location: lineStart, length: 0))
                let lineLength = contentsEnd - lineStart
                if lineLength > 0 {
                    let lineRange = NSRange(location: lineStart, length: lineLength)
                    let lineText = fullText.substring(with: lineRange)
                    if isFenceMarker(lineText) {
                        insideFence.toggle()
                    } else {
                        let decorations = MarkdownInlineHighlighter.decorations(
                            line: lineText,
                            isInsideFencedCode: insideFence
                        )
                        for decoration in decorations {
                            let location = lineStart + decoration.range.location
                            let length = decoration.range.length
                            guard location >= 0, length > 0, location + length <= fullText.length else { continue }
                            let nsRange = NSRange(location: location, length: length)
                            if let color = MarkdownInlineStyle.foregroundColor(for: decoration.kind) {
                                layoutManager.addTemporaryAttribute(
                                    .foregroundColor,
                                    value: color,
                                    forCharacterRange: nsRange
                                )
                            }
                            if let strike = MarkdownInlineStyle.strikethroughStyle(for: decoration.kind) {
                                layoutManager.addTemporaryAttribute(
                                    .strikethroughStyle,
                                    value: strike.rawValue,
                                    forCharacterRange: nsRange
                                )
                            }
                        }
                    }
                }
                if lineEnd <= lineStart { break }
                lineStart = lineEnd
            }
        }

        private func isFenceMarker(_ line: String) -> Bool {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            return trimmed.hasPrefix("```") || trimmed.hasPrefix("~~~")
        }
    }
}

final class MarkdownEditingTextView: NSTextView {
    var onSubmit: (() -> Void)?
    var onSubmitWithoutReturn: (() -> Void)?
    var onIncreaseFontSize: (() -> Void)?
    var onDecreaseFontSize: (() -> Void)?
    var onPasteImageData: ((Data) -> Void)?
    var onPasteFileURL: ((URL) -> Void)?
    var pendingFocusGrab: Bool = false

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        guard pendingFocusGrab else { return }
        pendingFocusGrab = false
        grabFirstResponder()
    }

    override func keyDown(with event: NSEvent) {
        let store = KeyBindingStore.shared
        if store.combo(for: .submitRichInput).matches(event: event) {
            let callback = onSubmit
            Task { @MainActor in callback?() }
            return
        }
        if store.combo(for: .submitRichInputWithoutReturn).matches(event: event) {
            let callback = onSubmitWithoutReturn
            Task { @MainActor in callback?() }
            return
        }
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        if flags.subtracting(.shift) == .command {
            switch event.charactersIgnoringModifiers?.lowercased() {
            case "=",
                 "+":
                let callback = onIncreaseFontSize
                Task { @MainActor in callback?() }
                return
            case "-":
                let callback = onDecreaseFontSize
                Task { @MainActor in callback?() }
                return
            default:
                break
            }
        }
        super.keyDown(with: event)
    }

    func grabFirstResponder() {
        guard let window else {
            pendingFocusGrab = true
            return
        }
        DispatchQueue.main.async { [weak self, weak window] in
            guard let self, let window else { return }
            window.makeFirstResponder(self)
        }
    }

    override func paste(_ sender: Any?) {
        let pasteboard = NSPasteboard.general
        if pasteboard.canReadObject(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]),
           let urls = pasteboard.readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL]
        {
            for url in urls {
                onPasteFileURL?(url)
            }
            if !urls.isEmpty { return }
        }
        if pasteboard.string(forType: .string) == nil,
           pasteboard.canReadObject(forClasses: [NSImage.self], options: nil),
           let imageData = readImageData(from: pasteboard)
        {
            onPasteImageData?(imageData)
            return
        }
        pasteAsPlainText(sender)
    }

    private func readImageData(from pasteboard: NSPasteboard) -> Data? {
        if let data = pasteboard.data(forType: .png) { return data }
        if let data = pasteboard.data(forType: .tiff) { return data }
        if let image = NSImage(pasteboard: pasteboard), let data = image.tiffRepresentation {
            return data
        }
        return nil
    }

    override func performDragOperation(_ sender: any NSDraggingInfo) -> Bool {
        let pasteboard = sender.draggingPasteboard
        if let urls = pasteboard.readObjects(forClasses: [NSURL.self], options: nil) as? [URL], !urls.isEmpty {
            for url in urls {
                onPasteFileURL?(url)
            }
            return true
        }
        if let image = NSImage(pasteboard: pasteboard),
           let data = image.tiffRepresentation
        {
            onPasteImageData?(data)
            return true
        }
        return super.performDragOperation(sender)
    }
}
