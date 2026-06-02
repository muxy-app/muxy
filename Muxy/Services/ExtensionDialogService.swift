import AppKit

@MainActor
enum ExtensionDialogService {
    struct ConfirmRequest: Equatable {
        let title: String
        let message: String
        let buttons: [String]
        let defaultButton: String?
        let cancelButton: String?
        let style: NSAlert.Style
    }

    struct AlertRequest: Equatable {
        let title: String
        let message: String
        let style: NSAlert.Style
    }

    static func confirm(_ request: ConfirmRequest) async -> String? {
        let alert = NSAlert()
        alert.alertStyle = request.style
        alert.messageText = request.title
        alert.informativeText = request.message
        let buttons = request.buttons.map { alert.addButton(withTitle: $0) }
        let equivalents = keyEquivalents(for: request)
        for (button, equivalent) in zip(buttons, equivalents) {
            button.keyEquivalent = equivalent
        }
        let result = await runModal(alert)
        guard let index = buttonIndex(for: result, buttonCount: request.buttons.count) else {
            return nil
        }
        let label = request.buttons[index]
        if let cancel = request.cancelButton, label == cancel {
            return nil
        }
        return label
    }

    static func alert(_ request: AlertRequest) async {
        let alert = NSAlert()
        alert.alertStyle = request.style
        alert.messageText = request.title
        alert.informativeText = request.message
        alert.addButton(withTitle: "OK")
        _ = await runModal(alert)
    }

    static func makeConfirmRequest(args: [String: Any]) throws -> ConfirmRequest {
        let title = string(args, "title") ?? ""
        let message = string(args, "message") ?? ""
        guard !title.isEmpty || !message.isEmpty else {
            throw APIError.invalidArguments("dialog requires title or message")
        }
        var buttons = (args["buttons"] as? [Any])?.compactMap { $0 as? String } ?? []
        buttons = buttons.filter { !$0.isEmpty }
        if buttons.isEmpty {
            buttons = ["OK", "Cancel"]
        }
        let defaultButton = string(args, "default")
        return ConfirmRequest(
            title: title,
            message: message,
            buttons: orderedButtons(buttons, defaultLabel: defaultButton),
            defaultButton: defaultButton,
            cancelButton: string(args, "cancel"),
            style: style(from: string(args, "style"))
        )
    }

    static func makeAlertRequest(args: [String: Any]) throws -> AlertRequest {
        let title = string(args, "title") ?? ""
        let message = string(args, "message") ?? ""
        guard !title.isEmpty || !message.isEmpty else {
            throw APIError.invalidArguments("alert requires title or message")
        }
        return AlertRequest(
            title: title,
            message: message,
            style: style(from: string(args, "style"))
        )
    }

    static func keyEquivalents(for request: ConfirmRequest) -> [String] {
        request.buttons.enumerated().map { index, label in
            if label == request.cancelButton { return "\u{1B}" }
            if index == 0 { return "\r" }
            return ""
        }
    }

    private static func orderedButtons(_ buttons: [String], defaultLabel: String?) -> [String] {
        guard let defaultLabel, let index = buttons.firstIndex(of: defaultLabel), index != 0 else {
            return buttons
        }
        var reordered = buttons
        reordered.remove(at: index)
        reordered.insert(defaultLabel, at: 0)
        return reordered
    }

    private static func buttonIndex(for response: NSApplication.ModalResponse, buttonCount: Int) -> Int? {
        let index = response.rawValue - NSApplication.ModalResponse.alertFirstButtonReturn.rawValue
        guard index >= 0, index < buttonCount else { return nil }
        return index
    }

    private static func runModal(_ alert: NSAlert) async -> NSApplication.ModalResponse {
        guard let parent = parentWindow() else {
            return alert.runModal()
        }
        return await withCheckedContinuation { continuation in
            alert.beginSheetModal(for: parent) { response in
                continuation.resume(returning: response)
            }
        }
    }

    private static func parentWindow() -> NSWindow? {
        NSApp.windows.first { $0.identifier == ShortcutContext.mainWindowIdentifier }
    }

    private static func style(from raw: String?) -> NSAlert.Style {
        switch raw {
        case "warning": .warning
        case "critical": .critical
        default: .informational
        }
    }

    private static func string(_ args: [String: Any], _ key: String) -> String? {
        args[key] as? String
    }
}
