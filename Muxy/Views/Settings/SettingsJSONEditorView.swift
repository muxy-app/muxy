import AppKit
import SwiftUI

struct SettingsJSONEditorView: View {
    private enum Source: String, CaseIterable, Identifiable {
        case user
        case system

        var id: String { rawValue }

        var title: String {
            switch self {
            case .user: "User"
            case .system: "System Defaults"
            }
        }
    }

    @State private var source: Source = .user
    @State private var text = ""
    @State private var status: String?
    @State private var errorMessage: String?
    @State private var isSearchVisible = false
    @State private var searchText = ""
    @State private var selectedSearchMatchIndex = 0
    @State private var searchMatchCount = 0
    @FocusState private var isSearchFocused: Bool

    var body: some View {
        VStack(spacing: 0) {
            header
            SettingsDivider()
            editor
            footer
        }
        .onAppear(perform: reload)
        .onChange(of: source) { _, _ in reload() }
        .onChange(of: searchText) { _, _ in selectedSearchMatchIndex = 0 }
        .overlay(alignment: .topTrailing) {
            Button("Find") {
                showSearch()
            }
            .keyboardShortcut("f", modifiers: .command)
            .frame(width: 0, height: 0)
            .opacity(0)
            .accessibilityHidden(true)
        }
    }

    private var editor: some View {
        VStack(spacing: 0) {
            if isSearchVisible {
                searchBar
            }

            SettingsJSONTextView(
                text: editorText,
                isEditable: source == .user,
                searchText: searchText,
                selectedSearchMatchIndex: selectedSearchMatchIndex,
                searchMatchCount: $searchMatchCount,
                onFindRequested: showSearch
            )
            .background(SettingsStyle.background)
            .overlay(alignment: .topLeading) {
                if text.isEmpty {
                    Text("{}")
                        .font(.system(size: 12, design: .monospaced))
                        .foregroundStyle(SettingsStyle.mutedForeground)
                        .padding(16)
                        .allowsHitTesting(false)
                }
            }
        }
    }

    private var searchBar: some View {
        HStack(spacing: 8) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(SettingsStyle.mutedForeground)

            TextField("Search JSON", text: $searchText)
                .textFieldStyle(.plain)
                .font(.system(size: SettingsMetrics.labelFontSize))
                .focused($isSearchFocused)
                .onSubmit { selectNextSearchMatch() }
                .onExitCommand(perform: hideSearch)

            Text(searchStatusText)
                .font(.system(size: SettingsMetrics.footnoteFontSize, design: .monospaced))
                .foregroundStyle(SettingsStyle.mutedForeground)
                .frame(width: 56, alignment: .trailing)

            Button {
                selectPreviousSearchMatch()
            } label: {
                Image(systemName: "chevron.up")
            }
            .jsonSearchArrowButton()
            .disabled(searchMatchCount == 0)

            Button {
                selectNextSearchMatch()
            } label: {
                Image(systemName: "chevron.down")
            }
            .jsonSearchArrowButton()
            .disabled(searchMatchCount == 0)

            Button {
                hideSearch()
            } label: {
                Image(systemName: "xmark")
            }
            .buttonStyle(.borderless)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(SettingsStyle.sidebarBackground)
        .overlay(alignment: .bottom) {
            SettingsDivider()
        }
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 10) {
                Text("JSON Settings")
                    .font(.system(size: 20, weight: .semibold))
                Spacer()
                Picker("Source", selection: $source) {
                    ForEach(Source.allCases) { source in
                        Text(source.title).tag(source)
                    }
                }
                .pickerStyle(.segmented)
                .frame(width: 220)
                .tint(SettingsStyle.accent)
            }

            Text(description)
                .font(.system(size: SettingsMetrics.labelFontSize))
                .foregroundStyle(SettingsStyle.mutedForeground)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(16)
        .background(SettingsStyle.background)
    }

    private var footer: some View {
        HStack(spacing: 10) {
            if let errorMessage {
                Label(errorMessage, systemImage: "exclamationmark.triangle")
                    .font(.system(size: SettingsMetrics.footnoteFontSize))
                    .foregroundStyle(SettingsStyle.destructive)
                    .lineLimit(2)
            } else if let status {
                Label(status, systemImage: "checkmark.circle")
                    .font(.system(size: SettingsMetrics.footnoteFontSize))
                    .foregroundStyle(SettingsStyle.mutedForeground)
            } else {
                Text(SettingsJSONStore.userSettingsURL.path)
                    .font(.system(size: SettingsMetrics.footnoteFontSize, design: .monospaced))
                    .foregroundStyle(SettingsStyle.mutedForeground)
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .textSelection(.enabled)
            }

            Spacer()

            Button("Reload") {
                reload()
            }
            .controlSize(.small)

            if source == .user {
                Button("Prettify") {
                    prettify()
                }
                .controlSize(.small)

                Button("Reset from Current Settings") {
                    SettingsJSONStore.resetUserSettingsFile()
                    reload()
                }
                .controlSize(.small)

                Button("Apply") {
                    apply()
                }
                .controlSize(.small)
                .buttonStyle(.borderedProminent)
            }
        }
        .padding(12)
        .background(SettingsStyle.sidebarBackground)
        .overlay(alignment: .top) {
            SettingsDivider()
        }
    }

    private var editorText: Binding<String> {
        Binding(
            get: { text },
            set: { newValue in
                guard source == .user else { return }
                text = newValue
                errorMessage = nil
                status = nil
            }
        )
    }

    private var description: String {
        switch source {
        case .user:
            "Edit user settings as JSON. Present keys are applied as user overrides for known Muxy settings. "
                + "Unknown keys are preserved in the file but ignored by Muxy."
        case .system:
            "Read-only reference of the built-in defaults supported by the JSON editor. "
                + "Copy keys into User JSON to override them."
        }
    }

    private var searchStatusText: String {
        guard !searchText.isEmpty else { return "" }
        guard searchMatchCount > 0 else { return "0/0" }
        return "\(selectedSearchMatchIndex + 1)/\(searchMatchCount)"
    }

    private func reload() {
        switch source {
        case .user:
            text = SettingsJSONStore.loadUserSettingsText()
        case .system:
            text = SettingsJSONStore.systemSettingsText
        }
        status = nil
        errorMessage = nil
        selectedSearchMatchIndex = 0
    }

    private func apply() {
        do {
            try SettingsJSONStore.saveUserSettingsText(text)
            text = SettingsJSONStore.loadUserSettingsText()
            status = "Settings applied"
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
            status = nil
        }
    }

    private func prettify() {
        do {
            text = try SettingsJSONStore.prettifiedSettingsText(text)
            status = "JSON prettified"
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
            status = nil
        }
    }

    private func showSearch() {
        isSearchVisible = true
        DispatchQueue.main.async {
            isSearchFocused = true
        }
    }

    private func hideSearch() {
        isSearchVisible = false
        searchText = ""
        selectedSearchMatchIndex = 0
        isSearchFocused = false
    }

    private func selectNextSearchMatch() {
        guard searchMatchCount > 0 else { return }
        selectedSearchMatchIndex = (selectedSearchMatchIndex + 1) % searchMatchCount
    }

    private func selectPreviousSearchMatch() {
        guard searchMatchCount > 0 else { return }
        selectedSearchMatchIndex = (selectedSearchMatchIndex + searchMatchCount - 1) % searchMatchCount
    }
}

private extension View {
    func jsonSearchArrowButton() -> some View {
        buttonStyle(.plain)
            .font(.system(size: 10, weight: .semibold))
            .foregroundStyle(SettingsStyle.foreground)
            .frame(width: 22, height: 20)
            .background(SettingsStyle.sidebarBackground, in: RoundedRectangle(cornerRadius: 5))
            .overlay(
                RoundedRectangle(cornerRadius: 5)
                    .stroke(SettingsStyle.border, lineWidth: 1)
            )
            .contentShape(RoundedRectangle(cornerRadius: 5))
    }
}

private struct SettingsJSONTextView: NSViewRepresentable {
    @Binding var text: String
    let isEditable: Bool
    let searchText: String
    let selectedSearchMatchIndex: Int
    @Binding var searchMatchCount: Int
    let onFindRequested: () -> Void

    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSScrollView()
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = true
        scrollView.autohidesScrollers = true
        scrollView.drawsBackground = true
        scrollView.backgroundColor = SettingsStyle.nsBackground

        let textView = JSONHighlightingTextView()
        textView.delegate = context.coordinator
        textView.isRichText = false
        textView.isAutomaticQuoteSubstitutionEnabled = false
        textView.isAutomaticDashSubstitutionEnabled = false
        textView.isAutomaticTextReplacementEnabled = false
        textView.isAutomaticSpellingCorrectionEnabled = false
        textView.usesFindBar = false
        textView.allowsUndo = true
        textView.minSize = NSSize(width: 0, height: 0)
        textView.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        textView.isVerticallyResizable = true
        textView.isHorizontallyResizable = true
        textView.autoresizingMask = [.width]
        textView.textContainer?.containerSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        textView.textContainer?.widthTracksTextView = false
        textView.font = .monospacedSystemFont(ofSize: 12, weight: .regular)
        textView.textColor = SettingsStyle.nsForeground
        textView.insertionPointColor = SettingsStyle.nsForeground
        textView.backgroundColor = SettingsStyle.nsBackground
        textView.onFindRequested = onFindRequested
        scrollView.documentView = textView
        context.coordinator.textView = textView
        context.coordinator.apply(
            text: text,
            isEditable: isEditable,
            searchText: searchText,
            selectedSearchMatchIndex: selectedSearchMatchIndex
        )

        return scrollView
    }

    func updateNSView(_ scrollView: NSScrollView, context: Context) {
        scrollView.backgroundColor = SettingsStyle.nsBackground
        context.coordinator.apply(
            text: text,
            isEditable: isEditable,
            searchText: searchText,
            selectedSearchMatchIndex: selectedSearchMatchIndex
        )
        let count = context.coordinator.searchMatchCount
        if searchMatchCount != count {
            DispatchQueue.main.async {
                searchMatchCount = count
            }
        }
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(text: $text)
    }

    @MainActor
    final class Coordinator: NSObject, NSTextViewDelegate {
        @Binding private var text: String
        weak var textView: JSONHighlightingTextView?
        private var isApplying = false
        private(set) var searchMatchCount = 0

        init(text: Binding<String>) {
            _text = text
        }

        func apply(text newText: String, isEditable: Bool, searchText: String, selectedSearchMatchIndex: Int) {
            guard let textView else { return }
            isApplying = true
            textView.isEditable = isEditable
            textView.isSelectable = true
            textView.backgroundColor = SettingsStyle.nsBackground
            textView.textColor = SettingsStyle.nsForeground
            textView.insertionPointColor = SettingsStyle.nsForeground
            if textView.string != newText {
                let selectedRanges = textView.selectedRanges
                textView.string = newText
                textView.selectedRanges = selectedRanges
            }
            JSONSyntaxHighlighter.apply(to: textView)
            let matches = JSONSearchHighlighter.apply(to: textView, query: searchText, selectedIndex: selectedSearchMatchIndex)
            searchMatchCount = matches
            isApplying = false
        }

        func textDidChange(_ notification: Notification) {
            guard !isApplying, let textView = notification.object as? NSTextView else { return }
            text = textView.string
            JSONSyntaxHighlighter.apply(to: textView)
        }
    }
}

private final class JSONHighlightingTextView: NSTextView {
    var onFindRequested: (() -> Void)?

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if event.modifierFlags.intersection(.deviceIndependentFlagsMask) == .command,
           event.charactersIgnoringModifiers?.lowercased() == "f"
        {
            onFindRequested?()
            return true
        }
        return super.performKeyEquivalent(with: event)
    }
}

private enum JSONSyntaxHighlighter {
    private static let tokenPattern = [
        #"("(?:\\.|[^"\\])*"\s*:)|("(?:\\.|[^"\\])*")"#,
        #"\b(true|false)\b|\bnull\b|(-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)|([{}\[\]:,])"#,
    ].joined()

    @MainActor
    static func apply(to textView: NSTextView) {
        let fullRange = NSRange(location: 0, length: (textView.string as NSString).length)
        guard let storage = textView.textStorage else { return }
        storage.beginEditing()
        storage.setAttributes(baseAttributes, range: fullRange)
        guard let regex = try? NSRegularExpression(pattern: tokenPattern) else {
            storage.endEditing()
            return
        }
        regex.enumerateMatches(in: textView.string, range: fullRange) { match, _, _ in
            guard let match else { return }
            let attributes = attributesForMatch(match)
            storage.addAttributes(attributes, range: match.range)
        }
        storage.endEditing()
    }

    @MainActor
    private static var baseAttributes: [NSAttributedString.Key: Any] {
        [
            .font: NSFont.monospacedSystemFont(ofSize: 12, weight: .regular),
            .foregroundColor: SettingsStyle.nsForeground,
            .backgroundColor: SettingsStyle.nsBackground,
        ]
    }

    @MainActor
    private static func attributesForMatch(_ match: NSTextCheckingResult) -> [NSAttributedString.Key: Any] {
        if match.range(at: 1).location != NSNotFound {
            return [.foregroundColor: NSColor(MuxyTheme.accent)]
        }
        if match.range(at: 2).location != NSNotFound {
            return [.foregroundColor: NSColor.systemGreen]
        }
        if match.range(at: 3).location != NSNotFound {
            return [.foregroundColor: NSColor.systemOrange]
        }
        if match.range(at: 4).location != NSNotFound {
            return [.foregroundColor: NSColor.systemPurple]
        }
        if match.range(at: 5).location != NSNotFound {
            return [.foregroundColor: NSColor.systemBlue]
        }
        return [.foregroundColor: SettingsStyle.mutedNSForeground]
    }
}

private enum JSONSearchHighlighter {
    @MainActor
    static func apply(to textView: NSTextView, query: String, selectedIndex: Int) -> Int {
        let layoutManager = textView.layoutManager
        let fullRange = NSRange(location: 0, length: (textView.string as NSString).length)
        layoutManager?.removeTemporaryAttribute(.backgroundColor, forCharacterRange: fullRange)

        let normalized = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return 0 }

        let ranges = matchRanges(in: textView.string, query: normalized)
        for (index, range) in ranges.enumerated() {
            let color = index == selectedIndex ? NSColor.systemYellow.withAlphaComponent(0.55) : NSColor.systemYellow
                .withAlphaComponent(0.22)
            layoutManager?.addTemporaryAttribute(.backgroundColor, value: color, forCharacterRange: range)
        }
        if ranges.indices.contains(selectedIndex) {
            textView.scrollRangeToVisible(ranges[selectedIndex])
        }
        return ranges.count
    }

    private static func matchRanges(in text: String, query: String) -> [NSRange] {
        let haystack = text as NSString
        var ranges: [NSRange] = []
        var searchRange = NSRange(location: 0, length: haystack.length)
        while searchRange.length > 0 {
            let range = haystack.range(of: query, options: [.caseInsensitive], range: searchRange)
            guard range.location != NSNotFound else { break }
            ranges.append(range)
            let nextLocation = range.location + max(range.length, 1)
            searchRange = NSRange(location: nextLocation, length: haystack.length - nextLocation)
        }
        return ranges
    }
}
