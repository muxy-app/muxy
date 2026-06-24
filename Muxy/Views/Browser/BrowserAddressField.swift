import AppKit
import SwiftUI

struct BrowserAddressField: NSViewRepresentable {
    @Binding var text: String
    @Binding var isFocused: Bool
    let model: BrowserSuggestionModel
    let suggestionsProvider: (String) -> [BrowserHistoryEntry]
    let onFocusClaimed: () -> Void
    let onSubmit: (BrowserHistoryEntry?, String) -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(
            text: $text,
            isFocused: $isFocused,
            model: model,
            suggestionsProvider: suggestionsProvider,
            onFocusClaimed: onFocusClaimed,
            onSubmit: onSubmit
        )
    }

    func makeNSView(context: Context) -> NSTextField {
        let field = NSTextField()
        field.isBordered = false
        field.drawsBackground = false
        field.focusRingType = .none
        field.font = .systemFont(ofSize: UIMetrics.fontBody)
        field.textColor = MuxyTheme.nsFg
        field.delegate = context.coordinator
        field.target = context.coordinator
        field.action = #selector(Coordinator.handleSubmit)
        field.cell?.sendsActionOnEndEditing = false
        field.placeholderAttributedString = NSAttributedString(
            string: "Search or enter address",
            attributes: [
                .foregroundColor: MuxyTheme.nsFgMuted,
                .font: NSFont.systemFont(ofSize: UIMetrics.fontBody),
            ]
        )
        context.coordinator.field = field
        return field
    }

    func updateNSView(_ field: NSTextField, context: Context) {
        if field.stringValue != text, !context.coordinator.isEditing {
            field.stringValue = text
        }
        context.coordinator.applyFocus(isFocused)
    }

    static func dismantleNSView(_: NSTextField, coordinator: Coordinator) {
        coordinator.tearDown()
    }

    @MainActor
    final class Coordinator: NSObject, NSTextFieldDelegate {
        private let text: Binding<String>
        private let isFocused: Binding<Bool>
        private let model: BrowserSuggestionModel
        private let suggestionsProvider: (String) -> [BrowserHistoryEntry]
        private let onFocusClaimed: () -> Void
        private let onSubmit: (BrowserHistoryEntry?, String) -> Void
        private var focusRequestID = 0

        weak var field: NSTextField?
        private(set) var isEditing = false
        private var panel: BrowserSuggestionPanel?
        private var clickMonitor: Any?
        private static let focusAttemptLimit = 12

        init(
            text: Binding<String>,
            isFocused: Binding<Bool>,
            model: BrowserSuggestionModel,
            suggestionsProvider: @escaping (String) -> [BrowserHistoryEntry],
            onFocusClaimed: @escaping () -> Void,
            onSubmit: @escaping (BrowserHistoryEntry?, String) -> Void
        ) {
            self.text = text
            self.isFocused = isFocused
            self.model = model
            self.suggestionsProvider = suggestionsProvider
            self.onFocusClaimed = onFocusClaimed
            self.onSubmit = onSubmit
        }

        func applyFocus(_ shouldFocus: Bool) {
            guard let field else { return }
            let isFirstResponder = field.currentEditor() != nil
            if shouldFocus {
                guard !isFirstResponder else { return }
                focusRequestID += 1
                claimFocus(for: field, requestID: focusRequestID, attempt: 0)
                return
            }
            focusRequestID += 1
            if isFirstResponder {
                field.window?.makeFirstResponder(nil)
            }
        }

        private func claimFocus(for field: NSTextField, requestID: Int, attempt: Int) {
            guard attempt < Self.focusAttemptLimit else { return }
            DispatchQueue.main.asyncAfter(deadline: .now() + focusRetryDelay(for: attempt)) { [weak self, weak field] in
                guard let self, let field else { return }
                guard self.focusRequestID == requestID, self.isFocused.wrappedValue else { return }
                guard let window = field.window else {
                    self.claimFocus(for: field, requestID: requestID, attempt: attempt + 1)
                    return
                }
                if field.currentEditor() != nil {
                    return
                }
                window.makeFirstResponder(field)
                guard field.currentEditor() == nil else { return }
                self.claimFocus(for: field, requestID: requestID, attempt: attempt + 1)
            }
        }

        private func focusRetryDelay(for attempt: Int) -> DispatchTimeInterval {
            if attempt == 0 {
                return .milliseconds(0)
            }
            return .milliseconds(min(120, attempt * 16))
        }

        func controlTextDidBeginEditing(_: Notification) {
            isEditing = true
            isFocused.wrappedValue = true
            onFocusClaimed()
            field?.currentEditor()?.selectAll(nil)
            refreshSuggestions()
        }

        func controlTextDidChange(_ notification: Notification) {
            guard let field = notification.object as? NSTextField else { return }
            text.wrappedValue = field.stringValue
            refreshSuggestions()
        }

        func controlTextDidEndEditing(_: Notification) {
            isEditing = false
            isFocused.wrappedValue = false
            dismissSuggestions()
        }

        func control(_: NSControl, textView _: NSTextView, doCommandBy selector: Selector) -> Bool {
            switch selector {
            case #selector(NSResponder.moveDown(_:)):
                guard !model.isEmpty else { return false }
                model.moveSelection(1)
                return true
            case #selector(NSResponder.moveUp(_:)):
                guard !model.isEmpty else { return false }
                model.moveSelection(-1)
                return true
            case #selector(NSResponder.cancelOperation(_:)):
                guard panel?.isVisible == true else { return false }
                dismissSuggestions()
                return true
            case #selector(NSResponder.insertNewline(_:)),
                 #selector(NSResponder.insertNewlineIgnoringFieldEditor(_:)):
                handleSubmit()
                return true
            default:
                return false
            }
        }

        @objc
        func handleSubmit() {
            let selected = model.activeEntry
            commit(selected)
        }

        private func refreshSuggestions() {
            guard let field else { return }
            let entries = suggestionsProvider(field.stringValue)
            model.update(entries)
            guard !entries.isEmpty else {
                dismissSuggestions()
                return
            }
            presentSuggestionsIfNeeded()
        }

        private func presentSuggestionsIfNeeded() {
            guard let field else { return }
            let panel = panel ?? makePanel()
            self.panel = panel
            panel.show(below: field, horizontalInset: UIMetrics.spacing4, verticalGap: UIMetrics.spacing2)
            installClickMonitorIfNeeded()
        }

        private func makePanel() -> BrowserSuggestionPanel {
            BrowserSuggestionPanel(model: model) { [weak self] entry in
                self?.accept(entry)
            }
        }

        private func accept(_ entry: BrowserHistoryEntry) {
            commit(entry)
        }

        private func commit(_ selected: BrowserHistoryEntry?) {
            let submittedText = field?.stringValue ?? text.wrappedValue
            if let selected {
                isEditing = false
                text.wrappedValue = selected.url
                field?.stringValue = selected.url
            } else {
                text.wrappedValue = submittedText
            }
            dismissSuggestions()
            onSubmit(selected, submittedText)
        }

        private func dismissSuggestions() {
            model.clear()
            panel?.hide()
            removeClickMonitor()
        }

        private func installClickMonitorIfNeeded() {
            guard clickMonitor == nil else { return }
            clickMonitor = NSEvent.addLocalMonitorForEvents(
                matching: [.leftMouseDown, .rightMouseDown, .otherMouseDown]
            ) { [weak self] event in
                self?.handleGlobalClick(event)
                return event
            }
        }

        private func handleGlobalClick(_ event: NSEvent) {
            guard let panel, panel.isVisible else { return }
            if event.window == field?.window || event.window?.parent == field?.window { return }
            dismissSuggestions()
        }

        private func removeClickMonitor() {
            guard let clickMonitor else { return }
            NSEvent.removeMonitor(clickMonitor)
            self.clickMonitor = nil
        }

        func tearDown() {
            focusRequestID += 1
            dismissSuggestions()
            panel = nil
        }
    }
}
