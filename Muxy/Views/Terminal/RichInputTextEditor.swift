import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct RichInputTextEditor: NSViewRepresentable {
    struct Configuration {
        var font: NSFont = .systemFont(ofSize: 13)
        var insets: NSSize = .init(width: 8, height: 8)
        var allowsUndo: Bool = true
        var lineWrapping: Bool = true
        var grabsFirstResponderOnAppear: Bool = false
        var lineHeightMultiplier: CGFloat = 1.0
    }

    struct Callbacks {
        var onSubmit: ((String?) -> Void)?
        var onSubmitWithoutReturn: ((String?) -> Void)?
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
        layoutManager.usesFontLeading = false
        let lineHeightDelegate = RichInputLineHeightDelegate(fallbackFont: configuration.font)
        lineHeightDelegate.lineHeightMultiplier = configuration.lineHeightMultiplier
        layoutManager.delegate = lineHeightDelegate
        context.coordinator.lineHeightDelegate = lineHeightDelegate
        layoutManager.addTextContainer(textContainer)

        let textStorage = NSTextStorage()
        textStorage.addLayoutManager(layoutManager)

        let textView = RichInputTextView(
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
        textView.font = configuration.font
        textView.textColor = Self.defaultForeground()
        textView.insertionPointColor = Self.defaultForeground()
        textView.string = text
        textView.delegate = context.coordinator
        textView.onSubmit = { callbacks.onSubmit?(context.coordinator.selectedText) }
        textView.onSubmitWithoutReturn = { callbacks.onSubmitWithoutReturn?(context.coordinator.selectedText) }
        textView.onIncreaseFontSize = callbacks.onIncreaseFontSize
        textView.onDecreaseFontSize = callbacks.onDecreaseFontSize
        textView.onPasteImageData = callbacks.onPasteImageData
        textView.onPasteFileURL = callbacks.onPasteFileURL
        context.coordinator.textView = textView

        scrollView.documentView = textView

        if configuration.grabsFirstResponderOnAppear {
            textView.pendingFocusGrab = true
        }

        DispatchQueue.main.async { [weak coordinator = context.coordinator] in
            coordinator?.reportContentHeight()
        }
        return scrollView
    }

    func updateNSView(_ scrollView: NSScrollView, context: Context) {
        guard let textView = scrollView.documentView as? RichInputTextView else { return }
        context.coordinator.parent = self
        if textView.string != text {
            textView.string = text
        }
        textView.font = configuration.font
        textView.textContainerInset = configuration.insets
        textView.textColor = Self.defaultForeground()
        textView.insertionPointColor = Self.defaultForeground()
        if let lineHeightDelegate = context.coordinator.lineHeightDelegate {
            lineHeightDelegate.fallbackFont = configuration.font
            if abs(lineHeightDelegate.lineHeightMultiplier - configuration.lineHeightMultiplier) > .ulpOfOne {
                lineHeightDelegate.lineHeightMultiplier = configuration.lineHeightMultiplier
                textView.layoutManager?.invalidateLayout(
                    forCharacterRange: NSRange(location: 0, length: textView.string.utf16.count),
                    actualCharacterRange: nil
                )
            }
        }
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
        var parent: RichInputTextEditor
        weak var textView: RichInputTextView?
        var lastFocusVersion: Int = -1
        var lineHeightDelegate: RichInputLineHeightDelegate?
        private var lastReportedHeight: CGFloat = -1

        init(parent: RichInputTextEditor) {
            self.parent = parent
        }

        func textDidChange(_ notification: Notification) {
            guard let textView = notification.object as? NSTextView else { return }
            parent.text = textView.string
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

        var selectedText: String? {
            guard let textView else { return nil }
            let range = textView.selectedRange()
            guard range.length > 0 else { return nil }
            let text = textView.string as NSString
            guard NSMaxRange(range) <= text.length else { return nil }
            return text.substring(with: range)
        }
    }
}

final class RichInputLineHeightDelegate: NSObject, NSLayoutManagerDelegate {
    var lineHeightMultiplier: CGFloat = 1.0
    var fallbackFont: NSFont

    init(fallbackFont: NSFont) {
        self.fallbackFont = fallbackFont
        super.init()
    }

    // swiftlint:disable:next function_parameter_count
    func layoutManager(
        _ layoutManager: NSLayoutManager,
        shouldSetLineFragmentRect lineFragmentRect: UnsafeMutablePointer<NSRect>,
        lineFragmentUsedRect: UnsafeMutablePointer<NSRect>,
        baselineOffset: UnsafeMutablePointer<CGFloat>,
        in textContainer: NSTextContainer,
        forGlyphRange glyphRange: NSRange
    ) -> Bool {
        guard lineHeightMultiplier > 1.0 + .ulpOfOne else { return false }

        let font = referenceFont(layoutManager: layoutManager, glyphRange: glyphRange)
        let ascent = font.ascender
        let descent = -font.descender
        let typographicHeight = ascent + descent
        let targetHeight = ceil(typographicHeight * lineHeightMultiplier)
        let paddingTop = (targetHeight - typographicHeight) / 2

        lineFragmentRect.pointee.size.height = targetHeight
        lineFragmentUsedRect.pointee.size.height = targetHeight
        baselineOffset.pointee = paddingTop + ascent
        return true
    }

    private func referenceFont(layoutManager: NSLayoutManager, glyphRange: NSRange) -> NSFont {
        if let storage = layoutManager.textStorage, glyphRange.length > 0 {
            let charIndex = layoutManager.characterIndexForGlyph(at: glyphRange.location)
            if charIndex < storage.length,
               let font = storage.attribute(.font, at: charIndex, effectiveRange: nil) as? NSFont
            {
                return font
            }
        }
        return fallbackFont
    }
}

final class RichInputTextView: NSTextView {
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
            if !urls.isEmpty {
                return
            }
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
        if let data = pasteboard.data(forType: .png) {
            return data
        }
        if let data = pasteboard.data(forType: .tiff) {
            return data
        }
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
