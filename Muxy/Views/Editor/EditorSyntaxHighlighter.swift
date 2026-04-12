import AppKit

@MainActor
final class EditorSyntaxHighlighter {
    private static let debounceDelay: TimeInterval = 0.15
    private static let editLineRadius = 3

    private var generation = 0
    private var activeTask: Task<Void, Never>?
    private var debounceWork: DispatchWorkItem?
    private var pendingEditLocation: Int?

    func cancel() {
        debounceWork?.cancel()
        debounceWork = nil
        activeTask?.cancel()
        activeTask = nil
        pendingEditLocation = nil
    }

    func scheduleHighlight(
        textView: NSTextView,
        fileExtension: String,
        onApply: @escaping (SyntaxHighlightResult, NSRange) -> Void
    ) {
        pendingEditLocation = textView.selectedRange().location
        debounceWork?.cancel()
        let work = DispatchWorkItem { [weak self, weak textView] in
            guard let self, let textView else { return }
            self.applyEditHighlight(
                textView: textView,
                fileExtension: fileExtension,
                onApply: onApply
            )
            self.pendingEditLocation = nil
        }
        debounceWork = work
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.debounceDelay, execute: work)
    }

    func applyFullHighlight(
        textView: NSTextView,
        range: NSRange,
        fileExtension: String,
        onApply: @escaping (SyntaxHighlightResult, NSRange) -> Void
    ) {
        guard range.length > 0 else { return }
        let text = textView.textStorage?.string ?? textView.string
        let gen = nextGeneration()
        let highlighter = SyntaxHighlightExtension(fileExtension: fileExtension)

        activeTask = Task { [weak self] in
            let result = await highlighter.computeHighlightsAsync(text: text, range: range)
            guard let self, self.generation == gen else { return }
            onApply(result, range)
        }
    }

    private func applyEditHighlight(
        textView: NSTextView,
        fileExtension: String,
        onApply: @escaping (SyntaxHighlightResult, NSRange) -> Void
    ) {
        guard let storage = textView.textStorage else { return }
        guard storage.length > 0 else { return }
        let content = storage.string as NSString
        let editLoc = pendingEditLocation ?? textView.selectedRange().location
        let safeLoc = min(editLoc, content.length)

        let editLineRange = content.lineRange(for: NSRange(location: safeLoc, length: 0))
        var startLoc = editLineRange.location
        var endLoc = NSMaxRange(editLineRange)

        for _ in 0 ..< Self.editLineRadius {
            if startLoc > 0 {
                let prev = content.lineRange(for: NSRange(location: max(0, startLoc - 1), length: 0))
                startLoc = prev.location
            }
            if endLoc < content.length {
                let next = content.lineRange(for: NSRange(location: min(endLoc, content.length - 1), length: 0))
                endLoc = NSMaxRange(next)
            }
        }

        let range = NSRange(location: startLoc, length: endLoc - startLoc)
        guard range.length > 0 else { return }

        let text = storage.string
        let gen = nextGeneration()
        let highlighter = SyntaxHighlightExtension(fileExtension: fileExtension)

        activeTask = Task { [weak self] in
            let result = await highlighter.computeHighlightsAsync(text: text, range: range)
            guard let self, self.generation == gen else { return }
            onApply(result, range)
        }
    }

    private func nextGeneration() -> Int {
        generation += 1
        activeTask?.cancel()
        activeTask = nil
        return generation
    }
}
