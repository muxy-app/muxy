import AppKit
import SwiftUI

struct PaletteOverlay<Item: Identifiable & Sendable>: View {
    struct Page {
        let items: [Item]
        let hasMore: Bool
    }

    static var searchDebounce: Duration { .milliseconds(120) }

    let placeholder: String
    let emptyLabel: String
    let noMatchLabel: String
    let pageSize: Int
    let revision: Int
    let isLoading: Bool
    let page: (String, Int, Int) -> Page
    let onSelect: (Item) -> Void
    let onDismiss: () -> Void
    let row: (Item, Bool) -> AnyView

    @State private var query = ""
    @State private var results: [Item] = []
    @State private var hasMore = false
    @State private var highlightedIndex: Int? = 0
    @State private var isSearching = false
    @State private var searchTask: Task<Void, Never>?
    @State private var refilterTask: Task<Void, Never>?

    var body: some View {
        ZStack {
            Color.black.opacity(0.3)
                .ignoresSafeArea()
                .onTapGesture { onDismiss() }

            OverlayPanel(width: UIMetrics.scaled(500), height: UIMetrics.scaled(380)) {
                VStack(spacing: 0) {
                    searchField
                    Divider().overlay(MuxyTheme.border)
                    resultsList
                }
            }
        }
        .onAppear {
            refilter()
        }
        .onChange(of: revision) {
            scheduleRefilter()
        }
        .onDisappear {
            searchTask?.cancel()
            refilterTask?.cancel()
        }
    }

    private var searchField: some View {
        HStack(spacing: UIMetrics.spacing4) {
            Image(systemName: "magnifyingglass")
                .foregroundStyle(MuxyTheme.fgMuted)
                .font(.system(size: UIMetrics.fontEmphasis))
                .accessibilityHidden(true)
            PaletteSearchField(
                text: $query,
                placeholder: placeholder,
                onSubmit: { confirmSelection() },
                onEscape: { onDismiss() },
                onArrowUp: { moveHighlight(-1) },
                onArrowDown: { moveHighlight(1) },
                onPageUp: { moveHighlight(-PaletteSearchField.pageJump) },
                onPageDown: { moveHighlight(PaletteSearchField.pageJump) }
            )
            if isLoading || isSearching {
                ProgressView()
                    .controlSize(.small)
                    .accessibilityLabel("Searching")
            }
        }
        .padding(.horizontal, UIMetrics.spacing6)
        .padding(.vertical, UIMetrics.spacing5)
        .onChange(of: query) {
            performSearch()
        }
    }

    private var resultsList: some View {
        Group {
            if results.isEmpty, !isLoading, !isSearching {
                VStack {
                    Spacer()
                    Text(query.isEmpty ? emptyLabel : noMatchLabel)
                        .font(.system(size: UIMetrics.fontBody))
                        .foregroundStyle(MuxyTheme.fgMuted)
                    Spacer()
                }
            } else {
                ScrollViewReader { proxy in
                    ScrollView(.vertical, showsIndicators: true) {
                        LazyVStack(spacing: 0) {
                            ForEach(Array(results.enumerated()), id: \.element.id) { index, item in
                                row(item, index == highlightedIndex)
                                    .contentShape(Rectangle())
                                    .onTapGesture { onSelect(item) }
                                    .id(item.id)
                                    .onAppear {
                                        if index >= results.count - 1 { loadMore() }
                                    }
                            }
                        }
                    }
                    .onChange(of: highlightedIndex) { _, newIndex in
                        guard let newIndex, newIndex < results.count else { return }
                        proxy.scrollTo(results[newIndex].id, anchor: nil)
                    }
                }
            }
        }
        .frame(maxHeight: .infinity)
    }

    private func performSearch() {
        searchTask?.cancel()
        let currentQuery = query
        isSearching = true

        searchTask = Task {
            try? await Task.sleep(for: Self.searchDebounce)
            guard !Task.isCancelled else { return }
            apply(page(currentQuery, 0, pageSize), resetHighlight: true)
            isSearching = false
        }
    }

    private func scheduleRefilter() {
        guard refilterTask == nil else { return }
        refilterTask = Task {
            try? await Task.sleep(for: Self.searchDebounce)
            refilterTask = nil
            guard !Task.isCancelled else { return }
            refilter()
        }
    }

    private func refilter() {
        searchTask?.cancel()
        searchTask = nil
        isSearching = false
        let limit = max(pageSize, results.count)
        apply(page(query, 0, limit), resetHighlight: results.isEmpty)
    }

    private func apply(_ result: Page, resetHighlight: Bool) {
        results = result.items
        hasMore = result.hasMore
        if resetHighlight || highlightedIndex == nil {
            highlightedIndex = result.items.isEmpty ? nil : 0
        } else if let index = highlightedIndex {
            highlightedIndex = min(index, max(0, result.items.count - 1))
        }
    }

    private func loadMore() {
        guard hasMore, !isSearching else { return }
        let next = page(query, results.count, pageSize)
        results.append(contentsOf: next.items)
        hasMore = next.hasMore
    }

    private func moveHighlight(_ delta: Int) {
        guard !results.isEmpty else { return }
        guard let current = highlightedIndex else {
            highlightedIndex = delta > 0 ? 0 : results.count - 1
            return
        }
        highlightedIndex = max(0, min(results.count - 1, current + delta))
    }

    private func confirmSelection() {
        guard let index = highlightedIndex, index < results.count else { return }
        onSelect(results[index])
    }
}

struct PaletteSearchField: NSViewRepresentable {
    static let pageJump = 10

    @Binding var text: String
    let placeholder: String
    var fontSize: CGFloat = UIMetrics.fontEmphasis
    let onSubmit: () -> Void
    let onEscape: () -> Void
    let onArrowUp: () -> Void
    let onArrowDown: () -> Void
    var onPageUp: () -> Void = {}
    var onPageDown: () -> Void = {}
    var onTab: () -> Void = {}
    var onBackTab: () -> Void = {}
    var onEmptyBackspace: () -> Void = {}
    var onControlKey: (String) -> Bool = { _ in false }

    func makeCoordinator() -> Coordinator {
        Coordinator(parent: self)
    }

    func makeNSView(context: Context) -> NSTextField {
        let field = PaletteNSTextField()
        field.delegate = context.coordinator
        field.isBordered = false
        field.drawsBackground = false
        field.focusRingType = .none
        field.font = .systemFont(ofSize: fontSize)
        field.textColor = NSColor(MuxyTheme.fg)
        field.placeholderString = placeholder
        field.cell?.sendsActionOnEndEditing = false
        field.onEscape = onEscape
        field.onControlKey = onControlKey
        claimFocus(for: field, attemptsRemaining: 5)
        return field
    }

    private func claimFocus(for field: NSTextField, attemptsRemaining: Int) {
        guard attemptsRemaining > 0 else { return }
        DispatchQueue.main.async {
            guard let window = field.window else {
                claimFocus(for: field, attemptsRemaining: attemptsRemaining - 1)
                return
            }
            if field.currentEditor() != nil {
                return
            }
            window.makeFirstResponder(field)
            guard field.currentEditor() == nil else { return }
            claimFocus(for: field, attemptsRemaining: attemptsRemaining - 1)
        }
    }

    func updateNSView(_ nsView: NSTextField, context: Context) {
        context.coordinator.parent = self
        if let editor = nsView.currentEditor() as? NSTextView {
            if editor.string != text {
                editor.string = text
                editor.selectedRange = NSRange(location: (text as NSString).length, length: 0)
            }
        } else if nsView.stringValue != text {
            nsView.stringValue = text
        }
        if nsView.placeholderString != placeholder {
            nsView.placeholderString = placeholder
        }
        if let field = nsView as? PaletteNSTextField {
            field.onEscape = onEscape
            field.onControlKey = onControlKey
        }
    }

    @MainActor
    final class Coordinator: NSObject, NSTextFieldDelegate {
        var parent: PaletteSearchField

        init(parent: PaletteSearchField) {
            self.parent = parent
        }

        func controlTextDidChange(_ obj: Notification) {
            guard let field = obj.object as? NSTextField else { return }
            syncText(from: field, skipsMarkedText: true)
        }

        func control(
            _ control: NSControl,
            textView _: NSTextView,
            doCommandBy commandSelector: Selector
        ) -> Bool {
            if commandSelector == #selector(NSResponder.insertNewline(_:)) {
                syncText(from: control, skipsMarkedText: false)
                parent.onSubmit()
                return true
            }
            if commandSelector == #selector(NSResponder.moveUp(_:)) {
                parent.onArrowUp()
                return true
            }
            if commandSelector == #selector(NSResponder.moveDown(_:)) {
                parent.onArrowDown()
                return true
            }
            if commandSelector == #selector(NSResponder.insertTab(_:)) {
                parent.onTab()
                return true
            }
            if commandSelector == #selector(NSResponder.insertBacktab(_:)) {
                parent.onBackTab()
                return true
            }
            if commandSelector == #selector(NSResponder.deleteBackward(_:)) {
                guard let field = control as? NSTextField, field.stringValue.isEmpty else { return false }
                parent.onEmptyBackspace()
                return true
            }
            return false
        }

        func syncText(from control: NSControl, skipsMarkedText: Bool) {
            let editor = control.currentEditor() as? NSTextView
            if skipsMarkedText, editor?.hasMarkedText() == true {
                return
            }
            let currentText = editor?.string ?? control.stringValue
            if parent.text != currentText {
                parent.text = currentText
            }
        }
    }
}

private final class PaletteNSTextField: NSTextField {
    var onEscape: (() -> Void)?
    var onControlKey: ((String) -> Bool)?

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if event.keyCode == 53 {
            onEscape?()
            return true
        }
        if handleControlKey(event) {
            return true
        }
        return super.performKeyEquivalent(with: event)
    }

    override func keyDown(with event: NSEvent) {
        if handleControlKey(event) { return }
        super.keyDown(with: event)
    }

    private func handleControlKey(_ event: NSEvent) -> Bool {
        guard event.modifierFlags.intersection(.deviceIndependentFlagsMask) == [.control],
              let key = event.charactersIgnoringModifiers?.lowercased()
        else { return false }
        return onControlKey?(key) == true
    }
}
