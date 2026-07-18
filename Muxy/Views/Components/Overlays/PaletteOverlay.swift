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
    let showsSearchToolbar: Bool
    let page: (String, ExtensionModalSearchOptions, Int, Int) -> Page
    let onSelect: (Item) -> Void
    let onDismiss: () -> Void
    let onQueryChange: ((String, ExtensionModalSearchOptions) -> Void)?
    let row: (Item, Bool) -> AnyView

    @State private var query = ""
    @State private var searchOptions = ExtensionModalSearchOptions()
    @State private var results: [Item] = []
    @State private var hasMore = false
    @State private var highlightedIndex: Int? = 0
    @State private var isSearching = false
    @State private var searchTask: Task<Void, Never>?
    @State private var refilterTask: Task<Void, Never>?
    @State private var queryChangeTask: Task<Void, Never>?
    @State private var submittedQuery = ""
    @State private var submittedOptions = ExtensionModalSearchOptions()
    @State private var isSearchFieldFocused = false
    @State private var keyMonitor: Any?
    @State private var paletteWindow: NSWindow?
    @State private var loadMoreGate = LoadMoreGate()

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
                    if showsSearchToolbar {
                        Divider().overlay(MuxyTheme.border)
                        searchToolbar
                    }
                }
            }
        }
        .onAppear {
            refilter()
            installKeyMonitor()
        }
        .onChange(of: revision) {
            scheduleRefilter()
        }
        .onDisappear {
            searchTask?.cancel()
            refilterTask?.cancel()
            queryChangeTask?.cancel()
            removeKeyMonitor()
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
                onSubmit: { submitSearchFieldQuery(query) },
                onSubmitText: { submitSearchFieldQuery($0) },
                onEscape: { onDismiss() },
                onArrowUp: { moveHighlight(-1) },
                onArrowDown: { moveHighlight(1) },
                onPageUp: { moveHighlight(-PaletteSearchField.pageJump) },
                onPageDown: { moveHighlight(PaletteSearchField.pageJump) },
                onHome: { highlightedIndex = results.isEmpty ? nil : 0 },
                onEnd: { highlightedIndex = results.isEmpty ? nil : results.count - 1 },
                onQueryChange: { newQuery in
                    guard onQueryChange != nil else { return }
                    scheduleQueryChange(newQuery)
                },
                onFocusChange: { isSearchFieldFocused = $0 },
                onWindowChange: { paletteWindow = $0 }
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
            if onQueryChange == nil {
                performSearch()
            } else {
                clearSubmittedResults()
            }
        }
    }

    private var searchToolbar: some View {
        HStack(spacing: UIMetrics.spacing3) {
            searchOptionButton("Aa", isOn: searchOptions.caseSensitive, help: "Match case") {
                searchOptions.caseSensitive.toggle()
                searchOptionsChanged()
            }
            searchOptionButton("W", isOn: searchOptions.wholeWord, help: "Match whole word") {
                searchOptions.wholeWord.toggle()
                searchOptionsChanged()
            }
            searchOptionButton(".*", isOn: searchOptions.regex, help: "Use regular expression") {
                searchOptions.regex.toggle()
                searchOptionsChanged()
            }
            Spacer()
        }
        .padding(.horizontal, UIMetrics.spacing6)
        .padding(.vertical, UIMetrics.spacing4)
    }

    private func searchOptionButton(
        _ title: String,
        isOn: Bool,
        help: String,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            Text(title)
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold, design: .monospaced))
                .foregroundStyle(isOn ? MuxyTheme.accentForeground : MuxyTheme.fgMuted)
                .frame(width: UIMetrics.scaled(30), height: UIMetrics.scaled(22))
                .background(isOn ? MuxyTheme.accent : MuxyTheme.hover)
                .clipShape(RoundedRectangle(cornerRadius: UIMetrics.scaled(5)))
        }
        .buttonStyle(.plain)
        .help(help)
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
                                        if index >= results.count - 1 {
                                            scheduleLoadMore()
                                        }
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
        let currentOptions = searchOptions
        isSearching = true

        searchTask = Task {
            try? await Task.sleep(for: Self.searchDebounce)
            guard !Task.isCancelled else { return }
            apply(page(currentQuery, currentOptions, 0, pageSize), resetHighlight: true)
            isSearching = false
        }
    }

    private func scheduleQueryChange(_ newQuery: String) {
        guard onQueryChange != nil else { return }
        queryChangeTask?.cancel()
        let currentQuery = newQuery
        let currentOptions = searchOptions
        queryChangeTask = Task {
            try? await Task.sleep(for: Self.searchDebounce)
            guard !Task.isCancelled else { return }
            submittedQuery = currentQuery
            submittedOptions = currentOptions
            isSearching = true
            onQueryChange?(currentQuery, currentOptions)
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
        apply(page(query, searchOptions, 0, limit), resetHighlight: results.isEmpty)
    }

    private func clearSubmittedResults() {
        guard query != submittedQuery || searchOptions != submittedOptions else { return }
        searchTask?.cancel()
        searchTask = nil
        results = []
        hasMore = false
        highlightedIndex = nil
        isSearching = false
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

    private func scheduleLoadMore() {
        guard hasMore, !isSearching, !loadMoreGate.isScheduled else { return }
        loadMoreGate.isScheduled = true
        Task { @MainActor in
            loadMoreGate.isScheduled = false
            loadMore()
        }
    }

    private func loadMore() {
        guard hasMore, !isSearching else { return }
        let next = page(query, searchOptions, results.count, pageSize)
        results.append(contentsOf: next.items)
        hasMore = next.hasMore
    }

    private func searchOptionsChanged() {
        searchTask?.cancel()
        searchTask = nil
        queryChangeTask?.cancel()
        queryChangeTask = nil
        isSearching = false
        guard onQueryChange != nil else {
            performSearch()
            return
        }
        scheduleQueryChange(query)
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

    private func submitSearchFieldQuery(_ submittedText: String) {
        guard let onQueryChange else {
            confirmSelection()
            return
        }
        if canConfirmSubmittedResult(submittedText) {
            confirmSelection()
            return
        }
        submittedQuery = submittedText
        submittedOptions = searchOptions
        if query != submittedText {
            query = submittedText
        }
        isSearching = true
        onQueryChange(submittedText, searchOptions)
    }

    private func canConfirmSubmittedResult(_ submittedText: String) -> Bool {
        guard submittedText == submittedQuery,
              searchOptions == submittedOptions,
              !isSearching,
              !isLoading,
              let index = highlightedIndex,
              index < results.count
        else { return false }
        return true
    }

    private func installKeyMonitor() {
        guard keyMonitor == nil else { return }
        keyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            if handleKeyEvent(event) {
                return nil
            }
            return event
        }
    }

    private func removeKeyMonitor() {
        guard let keyMonitor else { return }
        NSEvent.removeMonitor(keyMonitor)
        self.keyMonitor = nil
    }

    private func handleKeyEvent(_ event: NSEvent) -> Bool {
        if let paletteWindow, event.window !== paletteWindow {
            return false
        }
        switch PaletteOverlayKeyboard.action(
            forKeyCode: event.keyCode,
            firstResponder: event.window?.firstResponder,
            isSearchFieldFocused: isSearchFieldFocused
        ) {
        case .confirmSelection:
            confirmSelection()
            return true
        case .pageUp:
            moveHighlight(-PaletteSearchField.pageJump)
            return true
        case .pageDown:
            moveHighlight(PaletteSearchField.pageJump)
            return true
        case nil:
            return false
        }
    }
}

private final class LoadMoreGate {
    var isScheduled = false
}

enum PaletteOverlayKeyAction: Equatable {
    case confirmSelection
    case pageUp
    case pageDown
}

enum PaletteOverlayKeyboard {
    static func action(
        forKeyCode keyCode: UInt16,
        firstResponder: NSResponder?,
        isSearchFieldFocused: Bool
    ) -> PaletteOverlayKeyAction? {
        if isTextEditingResponder(firstResponder), keyCode == 36 || keyCode == 76 {
            return nil
        }

        switch keyCode {
        case 36 where !isSearchFieldFocused,
             76 where !isSearchFieldFocused:
            return .confirmSelection
        case 116:
            return .pageUp
        case 121:
            return .pageDown
        default:
            return nil
        }
    }

    private static func isTextEditingResponder(_ responder: NSResponder?) -> Bool {
        responder is NSTextView
    }
}

struct PaletteSearchField: NSViewRepresentable {
    static let pageJump = 10
    private static let focusAttemptLimit = 12

    @Binding var text: String
    let placeholder: String
    var focusRequest = 0
    var isEnabled = true
    var fontSize: CGFloat = UIMetrics.fontEmphasis
    let onSubmit: () -> Void
    var onSubmitText: ((String) -> Void)?
    let onEscape: () -> Void
    let onArrowUp: () -> Void
    let onArrowDown: () -> Void
    var onPageUp: () -> Void = {}
    var onPageDown: () -> Void = {}
    var onHome: () -> Void = {}
    var onEnd: () -> Void = {}
    var onQueryChange: ((String) -> Void)?
    var onTab: () -> Void = {}
    var onBackTab: () -> Void = {}
    var onEmptyBackspace: () -> Void = {}
    var onControlKey: (String) -> Bool = { _ in false }
    var onFocusChange: (Bool) -> Void = { _ in }
    var onWindowChange: (NSWindow?) -> Void = { _ in }

    func makeCoordinator() -> Coordinator {
        Coordinator(parent: self)
    }

    func makeNSView(context: Context) -> NSTextField {
        let field = PaletteNSTextField()
        field.delegate = context.coordinator
        field.isBordered = false
        field.drawsBackground = false
        field.focusRingType = .none
        field.isEnabled = isEnabled
        field.font = .systemFont(ofSize: fontSize)
        field.textColor = NSColor(MuxyTheme.fg)
        field.placeholderString = placeholder
        field.cell?.sendsActionOnEndEditing = false
        field.onEscape = onEscape
        field.onControlKey = onControlKey
        claimFocus(for: field, attempt: 0)
        return field
    }

    private func claimFocus(for field: NSTextField, attempt: Int) {
        guard attempt < Self.focusAttemptLimit else { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + focusRetryDelay(for: attempt)) { [weak field] in
            guard let field else { return }
            guard let window = field.window else {
                claimFocus(for: field, attempt: attempt + 1)
                return
            }
            if field.currentEditor() != nil {
                return
            }
            window.makeFirstResponder(field)
            guard field.currentEditor() == nil else { return }
            claimFocus(for: field, attempt: attempt + 1)
        }
    }

    private func focusRetryDelay(for attempt: Int) -> DispatchTimeInterval {
        if attempt == 0 {
            return .milliseconds(0)
        }
        return .milliseconds(min(120, attempt * 16))
    }

    func updateNSView(_ nsView: NSTextField, context: Context) {
        context.coordinator.parent = self
        if context.coordinator.lastFocusRequest != focusRequest {
            context.coordinator.lastFocusRequest = focusRequest
            claimFocus(for: nsView, attempt: 0)
        }
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
        if nsView.isEnabled != isEnabled {
            nsView.isEnabled = isEnabled
        }
        if let field = nsView as? PaletteNSTextField {
            field.onEscape = onEscape
            field.onControlKey = onControlKey
        }
        context.coordinator.notifyWindowChange(nsView.window)
    }

    @MainActor
    final class Coordinator: NSObject, NSTextFieldDelegate {
        var parent: PaletteSearchField
        var lastFocusRequest: Int
        private weak var lastNotifiedWindow: NSWindow?

        init(parent: PaletteSearchField) {
            self.parent = parent
            lastFocusRequest = parent.focusRequest
        }

        func notifyWindowChange(_ window: NSWindow?) {
            guard window !== lastNotifiedWindow else { return }
            lastNotifiedWindow = window
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                self.parent.onWindowChange(window)
            }
        }

        func controlTextDidChange(_ obj: Notification) {
            guard let field = obj.object as? NSTextField else { return }
            let currentText = syncText(from: field, skipsMarkedText: true)

            if let editor = field.currentEditor() as? NSTextView, editor.hasMarkedText() {
                return
            }
            if let field = field as? PaletteNSTextField, field.consumeSubmitAfterMarkedTextCommit() {
                submit(currentText)
                return
            }
            parent.onQueryChange?(field.stringValue)
        }

        func controlTextDidBeginEditing(_ obj: Notification) {
            parent.onFocusChange(true)
            notifyWindowChange((obj.object as? NSControl)?.window)
        }

        func controlTextDidEndEditing(_: Notification) {
            parent.onFocusChange(false)
        }

        func control(
            _ control: NSControl,
            textView: NSTextView,
            doCommandBy commandSelector: Selector
        ) -> Bool {
            if commandSelector == #selector(NSResponder.cancelOperation(_:)) {
                parent.onEscape()
                return true
            }
            if commandSelector == #selector(NSResponder.insertNewline(_:)) {
                if textView.hasMarkedText() {
                    (control as? PaletteNSTextField)?.deferSubmitAfterMarkedTextCommit()
                    return false
                }
                let submittedText = syncText(from: control, skipsMarkedText: false)
                if let field = control as? PaletteNSTextField {
                    _ = field.consumeSubmitAfterMarkedTextCommit()
                }
                submit(submittedText)
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
            if commandSelector == #selector(NSResponder.pageUp(_:))
                || commandSelector == #selector(NSResponder.scrollPageUp(_:))
                || commandSelector == Selector(("movePageUp:"))
            {
                parent.onPageUp()
                return true
            }
            if commandSelector == #selector(NSResponder.pageDown(_:))
                || commandSelector == #selector(NSResponder.scrollPageDown(_:))
                || commandSelector == Selector(("movePageDown:"))
            {
                parent.onPageDown()
                return true
            }
            if commandSelector == #selector(NSResponder.moveToBeginningOfLine(_:)) {
                parent.onHome()
                return true
            }
            if commandSelector == #selector(NSResponder.moveToEndOfLine(_:)) {
                parent.onEnd()
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

        private func submit(_ submittedText: String) {
            if let onSubmitText = parent.onSubmitText {
                onSubmitText(submittedText)
            } else {
                parent.onSubmit()
            }
        }

        @discardableResult
        func syncText(from control: NSControl, skipsMarkedText: Bool) -> String {
            let editor = control.currentEditor() as? NSTextView
            let currentText = editor?.string ?? control.stringValue
            if skipsMarkedText, editor?.hasMarkedText() == true {
                return currentText
            }
            if parent.text != currentText {
                parent.text = currentText
            }
            return currentText
        }
    }
}

final class PaletteNSTextField: NSTextField {
    var onEscape: (() -> Void)?
    var onControlKey: ((String) -> Bool)?
    private var submitAfterMarkedTextCommit = false

    func deferSubmitAfterMarkedTextCommit() {
        submitAfterMarkedTextCommit = true
    }

    func consumeSubmitAfterMarkedTextCommit() -> Bool {
        let shouldSubmit = submitAfterMarkedTextCommit
        submitAfterMarkedTextCommit = false
        return shouldSubmit
    }

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
        if handleControlKey(event) {
            return
        }
        if Self.isReturnKey(event),
           let editor = currentEditor() as? NSTextView,
           editor.hasMarkedText()
        {
            deferSubmitAfterMarkedTextCommit()
        }
        super.keyDown(with: event)
    }

    private static func isReturnKey(_ event: NSEvent) -> Bool {
        event.keyCode == 36 || event.keyCode == 76
    }

    private func handleControlKey(_ event: NSEvent) -> Bool {
        guard event.modifierFlags.intersection(.deviceIndependentFlagsMask) == [.control],
              let key = event.charactersIgnoringModifiers?.lowercased()
        else { return false }
        return onControlKey?(key) == true
    }
}
