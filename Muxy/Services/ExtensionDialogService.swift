import AppKit

@MainActor
enum ExtensionDialogService {
    struct ConfirmRequest: Equatable {
        let extensionID: String
        let title: String
        let message: String
        let buttons: [String]
        let defaultButton: String?
        let cancelButton: String?
        let style: NSAlert.Style
    }

    struct AlertRequest: Equatable {
        let extensionID: String
        let title: String
        let message: String
        let style: NSAlert.Style
    }

    struct PromptRequest: Equatable {
        let extensionID: String
        let title: String
        let message: String
        let defaultValue: String
        let placeholder: String
        let confirmButton: String
        let cancelButton: String
    }

    struct PickFolderRequest: Equatable {
        let extensionID: String
        let title: String
        let message: String
        let defaultPath: String?
    }

    static let maxTextLength = 2000
    static let maxButtonCount = 3

    private static var busyExtensionIDs: Set<String> = []

    static func confirm(_ request: ConfirmRequest) async throws -> String? {
        try claim(request.extensionID)
        defer { release(request.extensionID) }
        let alert = makeAlert(title: request.title, message: request.message, style: request.style)
        let buttons = request.buttons.map { alert.addButton(withTitle: $0) }
        let equivalents = keyEquivalents(for: request)
        for (button, equivalent) in zip(buttons, equivalents) {
            button.keyEquivalent = equivalent
        }
        let result = try await runModal(alert)
        guard let index = buttonIndex(for: result, buttonCount: request.buttons.count) else {
            return nil
        }
        let label = request.buttons[index]
        if let cancel = request.cancelButton, label == cancel {
            return nil
        }
        return label
    }

    static func alert(_ request: AlertRequest) async throws {
        try claim(request.extensionID)
        defer { release(request.extensionID) }
        let alert = makeAlert(title: request.title, message: request.message, style: request.style)
        alert.addButton(withTitle: "OK")
        _ = try await runModal(alert)
    }

    static func prompt(_ request: PromptRequest) async throws -> String? {
        try claim(request.extensionID)
        defer { release(request.extensionID) }
        let alert = makeAlert(title: request.title, message: request.message, style: .informational)
        let confirm = alert.addButton(withTitle: request.confirmButton)
        let cancel = alert.addButton(withTitle: request.cancelButton)
        confirm.keyEquivalent = "\r"
        cancel.keyEquivalent = "\u{1B}"
        let field = NSTextField(frame: NSRect(x: 0, y: 0, width: 260, height: 24))
        field.stringValue = request.defaultValue
        field.placeholderString = request.placeholder
        alert.accessoryView = field
        alert.window.initialFirstResponder = field
        let result = try await runModal(alert)
        guard result == .alertFirstButtonReturn else { return nil }
        return clamped(field.stringValue)
    }

    static func pickFolder(_ request: PickFolderRequest) async throws -> String? {
        try claim(request.extensionID)
        defer { release(request.extensionID) }
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.canCreateDirectories = true
        if !request.title.isEmpty { panel.message = request.title }
        if !request.message.isEmpty { panel.prompt = request.message }
        if let defaultPath = request.defaultPath {
            panel.directoryURL = URL(fileURLWithPath: defaultPath, isDirectory: true)
        }
        let response = try await runPanel(panel)
        guard response == .OK else { return nil }
        return panel.url?.path
    }

    static func makePromptRequest(extensionID: String, args: [String: Any]) throws -> PromptRequest {
        let title = clamped(string(args, "title") ?? "")
        let message = clamped(string(args, "message") ?? "")
        guard !title.isEmpty || !message.isEmpty else {
            throw APIError.invalidArguments("prompt requires title or message")
        }
        return PromptRequest(
            extensionID: extensionID,
            title: title,
            message: message,
            defaultValue: clamped(string(args, "default") ?? ""),
            placeholder: clamped(string(args, "placeholder") ?? ""),
            confirmButton: string(args, "confirm").map(clamped) ?? "OK",
            cancelButton: string(args, "cancel").map(clamped) ?? "Cancel"
        )
    }

    static func makePickFolderRequest(extensionID: String, args: [String: Any]) throws -> PickFolderRequest {
        PickFolderRequest(
            extensionID: extensionID,
            title: clamped(string(args, "title") ?? ""),
            message: clamped(string(args, "message") ?? ""),
            defaultPath: string(args, "default").map { ($0 as NSString).expandingTildeInPath }
        )
    }

    static func makeConfirmRequest(extensionID: String, args: [String: Any]) throws -> ConfirmRequest {
        let title = clamped(string(args, "title") ?? "")
        let message = clamped(string(args, "message") ?? "")
        guard !title.isEmpty || !message.isEmpty else {
            throw APIError.invalidArguments("dialog requires title or message")
        }
        var buttons = (args["buttons"] as? [Any])?.compactMap { $0 as? String } ?? []
        buttons = buttons.map(clamped).filter { !$0.isEmpty }
        if buttons.isEmpty {
            buttons = ["OK", "Cancel"]
        }
        buttons = Array(buttons.prefix(maxButtonCount))
        let defaultButton = string(args, "default").map(clamped)
        return ConfirmRequest(
            extensionID: extensionID,
            title: title,
            message: message,
            buttons: orderedButtons(buttons, defaultLabel: defaultButton),
            defaultButton: defaultButton,
            cancelButton: string(args, "cancel").map(clamped),
            style: style(from: string(args, "style"))
        )
    }

    static func makeAlertRequest(extensionID: String, args: [String: Any]) throws -> AlertRequest {
        let title = clamped(string(args, "title") ?? "")
        let message = clamped(string(args, "message") ?? "")
        guard !title.isEmpty || !message.isEmpty else {
            throw APIError.invalidArguments("alert requires title or message")
        }
        return AlertRequest(
            extensionID: extensionID,
            title: title,
            message: message,
            style: style(from: string(args, "style"))
        )
    }

    static func keyEquivalents(for request: ConfirmRequest) -> [String] {
        request.buttons.enumerated().map { index, label in
            if index == 0 { return "\r" }
            if label == request.cancelButton { return "\u{1B}" }
            return ""
        }
    }

    private static func makeAlert(title: String, message: String, style: NSAlert.Style) -> NSAlert {
        let alert = NSAlert()
        alert.alertStyle = style
        alert.messageText = title
        alert.informativeText = message
        return alert
    }

    private static func claim(_ extensionID: String) throws {
        guard !busyExtensionIDs.contains(extensionID) else {
            throw APIError.invalidArguments("a dialog is already open for this extension")
        }
        busyExtensionIDs.insert(extensionID)
    }

    private static func release(_ extensionID: String) {
        busyExtensionIDs.remove(extensionID)
    }

    private static func clamped(_ value: String) -> String {
        String(value.prefix(maxTextLength))
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

    private static func runModal(_ alert: NSAlert) async throws -> NSApplication.ModalResponse {
        guard let parent = parentWindow() else {
            throw APIError.invalidArguments("no window available to present the dialog")
        }
        return await withCheckedContinuation { continuation in
            alert.beginSheetModal(for: parent) { response in
                continuation.resume(returning: response)
            }
        }
    }

    private static func runPanel(_ panel: NSOpenPanel) async throws -> NSApplication.ModalResponse {
        guard let parent = parentWindow() else {
            throw APIError.invalidArguments("no window available to present the dialog")
        }
        return await withCheckedContinuation { continuation in
            panel.beginSheetModal(for: parent) { response in
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
