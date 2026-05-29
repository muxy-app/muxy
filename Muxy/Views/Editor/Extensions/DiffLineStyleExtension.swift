import AppKit

@MainActor
final class DiffGutterExtension: EditorExtension {
    let identifier = "diff-gutter"

    private weak var host: LineNumberGutterHost?
    private var gutterView: DiffGutterView?
    private var scrollObserver: NSObjectProtocol?

    init(host: LineNumberGutterHost) {
        self.host = host
    }

    func didMount(context: EditorRenderContext) {
        install(context: context)
    }

    func willUnmount(context _: EditorRenderContext) {
        remove()
    }

    func renderViewport(context: EditorRenderContext, lineRange _: Range<Int>) {
        ensureInstalled(context: context)
        update(context: context)
    }

    func geometryDidChange(context: EditorRenderContext) {
        update(context: context)
    }

    private func ensureInstalled(context: EditorRenderContext) {
        guard gutterView == nil else { return }
        install(context: context)
    }

    private func install(context: EditorRenderContext) {
        guard gutterView == nil,
              let host,
              let container = host.scrollContainer,
              let scrollView = host.scrollView
        else { return }
        let view = DiffGutterView()
        view.scrollView = scrollView
        applyState(to: view, context: context)
        container.setGutter(view)
        gutterView = view
        scrollView.contentView.postsBoundsChangedNotifications = true
        scrollObserver = NotificationCenter.default.addObserver(
            forName: NSView.boundsDidChangeNotification,
            object: scrollView.contentView,
            queue: .main
        ) { [weak view] _ in
            MainActor.assumeIsolated { view?.needsDisplay = true }
        }
    }

    private func remove() {
        if let scrollObserver {
            NotificationCenter.default.removeObserver(scrollObserver)
            self.scrollObserver = nil
        }
        host?.scrollContainer?.setGutter(nil)
        gutterView = nil
    }

    private func update(context: EditorRenderContext) {
        guard let view = gutterView else { return }
        let oldWidth = view.preferredWidth
        applyState(to: view, context: context)
        if abs(view.preferredWidth - oldWidth) > 0.5 {
            host?.scrollContainer?.gutterWidthDidChange()
        }
        view.needsDisplay = true
    }

    private func applyState(to view: DiffGutterView, context: EditorRenderContext) {
        view.labelFont = gutterLabelFont(for: context)
        view.lines = context.state.diffGutterLines ?? []
        view.lineHeight = max(1, context.viewport.estimatedLineHeight)
        view.topInset = context.textView.textContainerInset.height
        view.heightMap = context.viewport.heightMap
        view.wrappingEnabled = host?.lineWrappingEnabled ?? false
    }

    private func gutterLabelFont(for context: EditorRenderContext) -> NSFont {
        let base = context.textView.font ?? context.editorSettings.resolvedFont
        let size = max(9, base.pointSize - 1)
        if base.isFixedPitch {
            return NSFont(name: base.fontName, size: size) ?? NSFont.monospacedSystemFont(ofSize: size, weight: .regular)
        }
        return NSFont.monospacedSystemFont(ofSize: size, weight: .regular)
    }
}

@MainActor
final class DiffGutterView: LineNumberGutterView {
    var lines: [DiffEditorGutterLine] = [] {
        didSet {
            updateLineMetadata()
        }
    }

    private let columnGap: CGFloat = 8
    private let horizontalPadding: CGFloat = 8
    private let changeStripeWidth: CGFloat = 3

    private var maxDigitCount = 2
    private var hasOldColumn = false
    private var hasNewColumn = false
    private func updateLineMetadata() {
        var maxNumber = 0
        var foundOld = false
        var foundNew = false
        for line in lines {
            if let oldLineNumber = line.oldLineNumber {
                maxNumber = max(maxNumber, oldLineNumber)
                foundOld = true
            }
            if let newLineNumber = line.newLineNumber {
                maxNumber = max(maxNumber, newLineNumber)
                foundNew = true
            }
        }
        maxDigitCount = max(2, String(max(1, maxNumber)).count)
        hasOldColumn = foundOld
        hasNewColumn = foundNew
    }

    override var preferredWidth: CGFloat {
        let sample = String(repeating: "0", count: maxDigitCount)
        let numberWidth = (sample as NSString).size(withAttributes: [.font: labelFont]).width
        let numberColumns = hasOldColumn && hasNewColumn ? numberWidth * 2 + columnGap : numberWidth
        return ceil(changeStripeWidth + horizontalPadding + numberColumns + horizontalPadding)
    }

    override func draw(_: NSRect) {
        guard let scrollView else { return }
        let scrollY = scrollView.contentView.bounds.origin.y
        EditorThemePalette.active.background.setFill()
        bounds.fill()
        if wrappingEnabled, heightMap != nil {
            drawWrapped(scrollY: scrollY)
        } else {
            drawUniform(scrollY: scrollY)
        }
        drawTrailingBorder()
    }

    private func drawUniform(scrollY: CGFloat) {
        guard lineHeight > 0, !lines.isEmpty else { return }
        let topDocY = scrollY
        let bottomDocY = scrollY + bounds.height
        let firstLine = max(0, Int(floor((topDocY - topInset) / lineHeight)))
        let lastLine = min(lines.count - 1, Int(ceil((bottomDocY - topInset) / lineHeight)))
        guard firstLine <= lastLine else { return }
        for line in firstLine ... lastLine {
            draw(line: line, y: topInset + CGFloat(line) * lineHeight - scrollY, height: lineHeight)
        }
    }

    private func drawWrapped(scrollY: CGFloat) {
        guard let heightMap, !lines.isEmpty else { return }
        let topDocY = max(0, scrollY - topInset)
        let bottomDocY = max(topDocY, scrollY + bounds.height - topInset)
        let firstLine = max(0, min(heightMap.lineAtY(topDocY).line, lines.count - 1))
        let lastLine = max(firstLine, min(heightMap.lineAtY(bottomDocY).line, lines.count - 1))
        for line in firstLine ... lastLine {
            draw(
                line: line,
                y: topInset + heightMap.heightAbove(line: line) - scrollY,
                height: max(lineHeight, heightMap.heightOfLine(line))
            )
        }
    }

    private func draw(line index: Int, y: CGFloat, height: CGFloat) {
        guard index < lines.count else { return }
        let line = lines[index]
        let background = backgroundColor(for: line.kind)
        background.setFill()
        NSRect(x: 0, y: y, width: bounds.width, height: height).fill()
        drawStripe(for: line.kind, y: y, height: height)
        drawLabels(for: line, y: y, height: height)
    }

    private func drawStripe(for kind: DiffDisplayRow.Kind, y: CGFloat, height: CGFloat) {
        guard let color = stripeColor(for: kind) else { return }
        color.setFill()
        NSRect(x: 0, y: y, width: changeStripeWidth, height: height).fill()
    }

    private func drawLabels(for line: DiffEditorGutterLine, y: CGFloat, height: CGFloat) {
        let color = foregroundColor(for: line.kind)
        let attributes: [NSAttributedString.Key: Any] = [.font: labelFont, .foregroundColor: color]
        let sample = String(repeating: "0", count: maxDigitCount)
        let numberWidth = (sample as NSString).size(withAttributes: [.font: labelFont]).width
        var x = changeStripeWidth + horizontalPadding
        let rowFrame = NSRect(x: 0, y: y, width: bounds.width, height: height)
        if hasOldColumn, hasNewColumn {
            draw(label: label(for: line.oldLineNumber), x: x, width: numberWidth, rowFrame: rowFrame, attributes: attributes)
            x += numberWidth + columnGap
            draw(label: label(for: line.newLineNumber), x: x, width: numberWidth, rowFrame: rowFrame, attributes: attributes)
        } else {
            draw(
                label: label(for: line.oldLineNumber ?? line.newLineNumber),
                x: x,
                width: numberWidth,
                rowFrame: rowFrame,
                attributes: attributes
            )
        }
    }

    private func draw(label: String, x: CGFloat, width: CGFloat, rowFrame: NSRect, attributes: [NSAttributedString.Key: Any]) {
        let string = label as NSString
        let size = string.size(withAttributes: attributes)
        let origin = NSPoint(x: x + max(0, width - size.width), y: rowFrame.minY + (rowFrame.height - size.height) / 2)
        guard origin.y + size.height >= 0, origin.y <= bounds.height else { return }
        string.draw(at: origin, withAttributes: attributes)
    }

    private func label(for value: Int?) -> String {
        value.map(String.init) ?? ""
    }

    private func foregroundColor(for kind: DiffDisplayRow.Kind) -> NSColor {
        switch kind {
        case .addition:
            MuxyTheme.nsDiffAdd
        case .deletion:
            MuxyTheme.nsDiffRemove
        case .hunk:
            EditorThemePalette.active.foreground.withAlphaComponent(0.75)
        case .collapsed:
            EditorThemePalette.active.foreground.withAlphaComponent(0.55)
        case .context,
             .commentSpacer:
            EditorThemePalette.active.foreground.withAlphaComponent(0.45)
        }
    }

    private func backgroundColor(for kind: DiffDisplayRow.Kind) -> NSColor {
        switch kind {
        case .addition:
            MuxyTheme.nsDiffAdd.withAlphaComponent(0.14)
        case .deletion:
            MuxyTheme.nsDiffRemove.withAlphaComponent(0.14)
        case .hunk:
            MuxyTheme.nsBg.blended(withFraction: 0.08, of: MuxyTheme.nsFg) ?? MuxyTheme.nsBg
        case .collapsed:
            EditorThemePalette.active.foreground.withAlphaComponent(0.08)
        case .context,
             .commentSpacer:
            EditorThemePalette.active.background
        }
    }

    private func stripeColor(for kind: DiffDisplayRow.Kind) -> NSColor? {
        switch kind {
        case .addition:
            MuxyTheme.nsDiffAdd
        case .deletion:
            MuxyTheme.nsDiffRemove
        case .context,
             .hunk,
             .collapsed,
             .commentSpacer:
            nil
        }
    }

    private func drawTrailingBorder() {
        EditorThemePalette.active.foreground.withAlphaComponent(0.08).setStroke()
        let path = NSBezierPath()
        path.move(to: NSPoint(x: bounds.maxX - 0.5, y: 0))
        path.line(to: NSPoint(x: bounds.maxX - 0.5, y: bounds.height))
        path.lineWidth = 1
        path.stroke()
    }
}

@MainActor
final class DiffLineStyleExtension: EditorExtension {
    let identifier = "diff-line-style"

    func renderViewport(context: EditorRenderContext, lineRange _: Range<Int>) {
        guard let kinds = context.state.diffLineKinds else { return }
        applyStyles(context: context, kinds: kinds)
    }

    private func applyStyles(context: EditorRenderContext, kinds: [DiffDisplayRow.Kind]) {
        let storageLength = context.storage.length
        guard storageLength > 0 else { return }

        let fullRange = NSRange(location: 0, length: storageLength)
        context.layoutManager.removeTemporaryAttribute(.backgroundColor, forCharacterRange: fullRange)
        let preservesSyntaxForeground = context.state.syntaxHighlighter != nil
        if !preservesSyntaxForeground {
            context.layoutManager.removeTemporaryAttribute(.foregroundColor, forCharacterRange: fullRange)
        }

        let viewportStart = context.viewport.viewportStartLine
        var runKind: DiffDisplayRow.Kind?
        var runStart: Int?
        var runEnd = 0

        for localLine in 0 ..< context.lineStartOffsets.count {
            let globalLine = viewportStart + localLine
            guard globalLine < kinds.count else { continue }
            let start = context.lineStartOffsets[localLine]
            let end = min(
                storageLength,
                localLine + 1 < context.lineStartOffsets.count ? context.lineStartOffsets[localLine + 1] : storageLength
            )
            guard end > start else { continue }
            let kind = kinds[globalLine]

            if runKind == kind, runEnd == start {
                runEnd = end
                continue
            }

            flushRun(
                kind: runKind,
                start: runStart,
                end: runEnd,
                preservesSyntaxForeground: preservesSyntaxForeground,
                layoutManager: context.layoutManager
            )
            runKind = kind
            runStart = start
            runEnd = end
        }

        flushRun(
            kind: runKind,
            start: runStart,
            end: runEnd,
            preservesSyntaxForeground: preservesSyntaxForeground,
            layoutManager: context.layoutManager
        )
    }

    private func flushRun(
        kind: DiffDisplayRow.Kind?,
        start: Int?,
        end: Int,
        preservesSyntaxForeground: Bool,
        layoutManager: NSLayoutManager
    ) {
        guard let kind, let start, end > start else { return }
        applyStyle(
            kind: kind,
            range: NSRange(location: start, length: end - start),
            preservesSyntaxForeground: preservesSyntaxForeground,
            layoutManager: layoutManager
        )
    }

    private func applyStyle(
        kind: DiffDisplayRow.Kind,
        range: NSRange,
        preservesSyntaxForeground: Bool,
        layoutManager: NSLayoutManager
    ) {
        switch kind {
        case .addition:
            applyFallbackForeground(
                MuxyTheme.nsDiffAdd,
                range: range,
                preservesSyntaxForeground: preservesSyntaxForeground,
                layoutManager: layoutManager
            )
        case .deletion:
            applyFallbackForeground(
                MuxyTheme.nsDiffRemove,
                range: range,
                preservesSyntaxForeground: preservesSyntaxForeground,
                layoutManager: layoutManager
            )
        case .hunk:
            layoutManager.addTemporaryAttribute(.foregroundColor, value: MuxyTheme.nsDiffHunk, forCharacterRange: range)
        case .collapsed:
            layoutManager.addTemporaryAttribute(
                .foregroundColor,
                value: EditorThemePalette.active.foreground.withAlphaComponent(0.55),
                forCharacterRange: range
            )
        case .context,
             .commentSpacer:
            break
        }
    }

    private func applyFallbackForeground(
        _ color: NSColor,
        range: NSRange,
        preservesSyntaxForeground: Bool,
        layoutManager: NSLayoutManager
    ) {
        guard !preservesSyntaxForeground else { return }
        layoutManager.addTemporaryAttribute(.foregroundColor, value: color, forCharacterRange: range)
    }
}
