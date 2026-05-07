import AppKit
import SwiftUI

/// A generic command-palette overlay with a search field, a scrollable
/// results list, and keyboard navigation. Used by Quick Open (files) and
/// the Worktree Switcher.
struct PaletteOverlay<Item: Identifiable & Sendable>: View {
    let placeholder: String
    let emptyLabel: String
    let noMatchLabel: String
    /// Provides items for a given query. Called on every query change.
    let search: (String) async -> [Item]
    let onSelect: (Item) -> Void
    let onDismiss: () -> Void
    let row: (Item, Bool) -> AnyView

    @State private var query = ""
    @State private var results: [Item] = []
    @State private var highlightedIndex: Int? = 0
    @State private var isSearching = false
    @State private var searchTask: Task<Void, Never>?

    var body: some View {
        ZStack {
            Color.black.opacity(0.3)
                .ignoresSafeArea()
                .onTapGesture { onDismiss() }

            VStack(spacing: 0) {
                searchField
                Divider().overlay(MuxyTheme.border)
                resultsList
            }
            .frame(width: UIMetrics.scaled(500), height: UIMetrics.scaled(380))
            .background(MuxyTheme.bg)
            .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusXL))
            .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusXL).stroke(MuxyTheme.border, lineWidth: 1))
            .shadow(color: .black.opacity(0.4), radius: UIMetrics.scaled(20), y: UIMetrics.scaled(8))
            .padding(.top, UIMetrics.scaled(60))
            .frame(maxHeight: .infinity, alignment: .top)
            .accessibilityAddTraits(.isModal)
        }
        .onAppear {
            performSearch(debounce: false)
        }
        .onDisappear {
            searchTask?.cancel()
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
                onArrowDown: { moveHighlight(1) }
            )
        }
        .padding(.horizontal, UIMetrics.spacing6)
        .padding(.vertical, UIMetrics.spacing5)
        .onChange(of: query) {
            performSearch()
        }
    }

    private var resultsList: some View {
        Group {
            if results.isEmpty, !isSearching {
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

    private func performSearch(debounce: Bool = true) {
        searchTask?.cancel()

        let currentQuery = query
        isSearching = true

        searchTask = Task {
            if debounce {
                try? await Task.sleep(for: .milliseconds(50))
                guard !Task.isCancelled else { return }
            }

            let found = await search(currentQuery)
            guard !Task.isCancelled else { return }

            await MainActor.run {
                results = found
                highlightedIndex = found.isEmpty ? nil : 0
                isSearching = false
            }
        }
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
    @Binding var text: String
    let placeholder: String
    var fontSize: CGFloat = UIMetrics.fontEmphasis
    let onSubmit: () -> Void
    let onEscape: () -> Void
    let onArrowUp: () -> Void
    let onArrowDown: () -> Void

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
        DispatchQueue.main.async {
            field.window?.makeFirstResponder(field)
        }
        return field
    }

    func updateNSView(_ nsView: NSTextField, context: Context) {
        context.coordinator.parent = self
        if nsView.stringValue != text {
            nsView.stringValue = text
        }
        if let field = nsView as? PaletteNSTextField {
            field.onEscape = onEscape
        }
    }

    final class Coordinator: NSObject, NSTextFieldDelegate {
        var parent: PaletteSearchField

        init(parent: PaletteSearchField) {
            self.parent = parent
        }

        func controlTextDidChange(_ obj: Notification) {
            guard let field = obj.object as? NSTextField else { return }
            parent.text = field.stringValue
        }

        func control(
            _ control: NSControl,
            textView _: NSTextView,
            doCommandBy commandSelector: Selector
        ) -> Bool {
            if commandSelector == #selector(NSResponder.insertNewline(_:)) {
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
            return false
        }
    }
}

private final class PaletteNSTextField: NSTextField {
    var onEscape: (() -> Void)?

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if event.keyCode == 53 {
            onEscape?()
            return true
        }
        return super.performKeyEquivalent(with: event)
    }
}
